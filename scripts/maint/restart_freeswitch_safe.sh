#!/bin/bash
#===============================================================================
# restart_freeswitch_safe.sh
#
# Restarts FreeSWITCH (the SBC's internal node) to activate settings that need a
# restart, with a low-traffic gate, verification and automatic rollback.
#
# Changes applied (idempotent):
#   - switch.conf.xml rtp-start-port 20002 / rtp-end-port 32768: already in the
#     file since 2026-10-22, only checked here (no overlap with RTPEngine 10000-20000).
#   - switch.conf.xml loglevel debug → info (~1 GB/day of log); also applied
#     live with `fsctl loglevel info` before the restart.
#   - with --timerfd: load mod_timerfd and set rtp-timer-name=timerfd on the
#     internal/external profiles (RTP TX clocked by the kernel, immune to the
#     scheduler jitter seen in June). Optional; leave it out to change less.
#
# Restarting drops every call in progress. The billing engine reconnects over
# ESL and reconciles orphan active_calls on its own.
#
# Usage: restart_freeswitch_safe.sh [--yes] [--force-now] [--timerfd] [--check-only]
# Env: MAX_CALLS (10)  WAIT_MAX_MIN (20)  STABLE_SECS (60)
#===============================================================================
set -uo pipefail

REPO=/opt/ApoloBilling
FS=/etc/freeswitch
PING="python3 -I $REPO/scripts/maint/sip_ping.py"
SIP_IP=10.118.0.6
GATEWAYS="kamailio freeswitch-legado sbc-push"

MAX_CALLS="${MAX_CALLS:-10}"
WAIT_MAX_MIN="${WAIT_MAX_MIN:-20}"
STABLE_SECS="${STABLE_SECS:-60}"
YES=0; FORCE=0; TIMERFD=0; CHECK_ONLY=0
for a in "$@"; do
    case "$a" in
        --yes) YES=1 ;;
        --force-now) FORCE=1 ;;
        --timerfd) TIMERFD=1 ;;
        --check-only) CHECK_ONLY=1 ;;
        *) echo "unknown option $a"; exit 2 ;;
    esac
done

TS=$(date +%Y%m%d-%H%M%S)
LOG=/var/log/apolo-maint-freeswitch-$TS.log
BACKUP=/root/freeswitch-conf-$TS.tgz
exec > >(tee -a "$LOG") 2>&1

log()  { echo "[$(date +%T)] $*"; }
fail() { log "ERROR: $*"; exit 1; }
fscli() { timeout 10 fs_cli -x "$1" 2>/dev/null; }
xml_ok() { python3 -I -c "import sys,xml.dom.minidom as m; [m.parse(f) for f in sys.argv[1:]]" "$@"; }
[ "$(id -u)" -eq 0 ] || fail "run as root"

#--- 1. pre-flight ---------------------------------------------------------------
fscli status | grep -q "is ready" || fail "FreeSWITCH is not answering fs_cli; not touching it"
REGS_BEFORE=$(fscli 'show registrations count' | awk '/total/{print $1}')
log "Registrations before: ${REGS_BEFORE:-?}"
grep -q '<param name="rtp-start-port" value="20002"/>' $FS/autoload_configs/switch.conf.xml \
    || fail "rtp-start-port 20002 not set in switch.conf.xml (expected since 2026-10-22)"
[ "$TIMERFD" -eq 1 ] && { [ -f /usr/lib/freeswitch/mod/mod_timerfd.so ] || fail "mod_timerfd.so not installed"; }

#--- 2. backup + config edits ----------------------------------------------------
tar czf "$BACKUP" -C / etc/freeswitch/autoload_configs etc/freeswitch/sip_profiles || fail "backup failed"
log "Backup: $BACKUP"
restore() { tar xzf "$BACKUP" -C / && chown -R freeswitch:freeswitch $FS/autoload_configs $FS/sip_profiles; }

sed -i 's#<param name="loglevel" value="debug"/>#<param name="loglevel" value="info"/>#' $FS/autoload_configs/switch.conf.xml
if [ "$TIMERFD" -eq 1 ]; then
    sed -i 's#<!-- *<load module="mod_timerfd"/> *-->#<load module="mod_timerfd"/>#' $FS/autoload_configs/modules.conf.xml
    for p in internal external; do
        f=$FS/sip_profiles/$p.xml
        grep -q 'name="rtp-timer-name"' "$f" \
            || sed -i '0,/<param name="rtp-ip"/s##<param name="rtp-timer-name" value="timerfd"/>\n      &#' "$f"
    done
fi
xml_ok $FS/autoload_configs/switch.conf.xml $FS/autoload_configs/modules.conf.xml \
       $FS/sip_profiles/internal.xml $FS/sip_profiles/external.xml \
    || { restore; fail "edited XML is invalid; config restored"; }
log "Changes vs backup:"
mkdir -p /tmp/fs-maint-$TS && tar xzf "$BACKUP" -C /tmp/fs-maint-$TS
diff -ru /tmp/fs-maint-$TS/etc/freeswitch/autoload_configs $FS/autoload_configs | grep '^[-+][^-+]'
diff -ru /tmp/fs-maint-$TS/etc/freeswitch/sip_profiles $FS/sip_profiles | grep '^[-+][^-+]'
if [ "$CHECK_ONLY" -eq 1 ]; then
    restore; log "--check-only: config restored, nothing restarted"; exit 0
fi
if [ "$YES" -eq 0 ]; then
    read -r -p "Restart FreeSWITCH now (drops calls in progress)? [y/N] " ans
    [ "$ans" = "y" ] || { restore; fail "aborted by operator; config restored"; }
fi
fscli 'fsctl loglevel info' >/dev/null

#--- 3. low-traffic gate ---------------------------------------------------------
calls() { fscli 'show calls count' | awk '/total/{print $1}'; }
if [ "$FORCE" -eq 0 ]; then
    deadline=$(( $(date +%s) + WAIT_MAX_MIN*60 ))
    while :; do
        n=$(calls); n=${n:-999}
        [ "$n" -le "$MAX_CALLS" ] && { log "Calls in progress: $n (<= $MAX_CALLS) → go"; break; }
        [ "$(date +%s)" -ge "$deadline" ] && { restore; fail "still $n calls after $WAIT_MAX_MIN min; config restored, nothing restarted"; }
        log "Calls in progress: $n > $MAX_CALLS, waiting 30s..."
        sleep 30
    done
fi

#--- 4-5. restart + verify -------------------------------------------------------
verify() {
    local pid0 pid1 st
    for i in $(seq 1 60); do fscli status | grep -q "is ready" && break; sleep 1; done
    fscli status | grep -q "is ready" || { log "fs_cli not answering 60s after start"; return 1; }
    sleep 3
    st=$(fscli 'sofia status')
    for p in internal external; do
        echo "$st" | grep -E "^ *$p +profile" | grep -q RUNNING || { log "profile $p not RUNNING"; return 1; }
    done
    for g in $GATEWAYS; do
        echo "$st" | grep -q "external::$g" || { log "gateway $g missing"; return 1; }
    done
    $PING $SIP_IP 5060 $SIP_IP >/dev/null || { log "no OPTIONS reply on :5060"; return 1; }
    $PING $SIP_IP 5062 $SIP_IP >/dev/null || { log "no OPTIONS reply on :5062"; return 1; }
    if [ "$TIMERFD" -eq 1 ]; then
        fscli 'module_exists mod_timerfd' | grep -q true || { log "mod_timerfd not loaded"; return 1; }
    fi
    pid0=$(systemctl show -p MainPID --value freeswitch)
    log "Stability window ${STABLE_SECS}s (MainPID $pid0)..."
    sleep "$STABLE_SECS"
    pid1=$(systemctl show -p MainPID --value freeswitch)
    [ "$pid0" = "$pid1" ] && fscli status | grep -q "is ready" || { log "FreeSWITCH restarted during the window"; return 1; }
    return 0
}

SINCE=$(date '+%F %T')
log "systemctl restart freeswitch"
systemctl restart freeswitch
if verify; then
    log "OK: FreeSWITCH up with the new config"
    log "Registrations now: $(fscli 'show registrations count' | awk '/total/{print $1}') (before: ${REGS_BEFORE:-?}; apps re-register within their expiry)"
    log "Billing engine ESL since restart:"
    timeout 20 journalctl -u apolo-billing-engine --since "$SINCE" --no-pager 2>/dev/null | grep -iE "connect|reconcile|ESL" | tail -5
    log "RTP ports in use by FreeSWITCH (lowest; must be >= 20002 once calls arrive):"
    ss -Huanp | grep '"freeswitch"' | awk '{n=split($4,a,":"); print a[n]}' | sort -n | awk '$1>=10000' | head -3
    log "Log: $LOG"
    exit 0
fi

#--- 6. rollback -----------------------------------------------------------------
log "VERIFICATION FAILED → restoring $BACKUP and restarting"
restore
systemctl restart freeswitch
if TIMERFD=0 STABLE_SECS=20 verify; then
    log "Rollback OK: FreeSWITCH running the previous config. Log: $LOG"
else
    log "ROLLBACK ALSO FAILED — intervene manually: journalctl -u freeswitch -n 100; fs_cli"
fi
exit 1
