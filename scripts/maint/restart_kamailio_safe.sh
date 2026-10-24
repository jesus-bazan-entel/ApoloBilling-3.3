#!/bin/bash
#===============================================================================
# restart_kamailio_safe.sh
#
# Installs config/kamailio/kamailio.cfg (repo) as /etc/kamailio/kamailio.cfg and
# restarts Kamailio with pre-flight validation, a low-traffic gate, post-restart
# verification and automatic rollback to the previous config.
#
# Steps:
#   1. Build candidate = repo cfg + DB credentials taken from the live cfg.
#   2. Show the diff live → candidate and ask for confirmation (unless --yes).
#   3. kamailio -c + test-start of a scratch copy (catches socket errors that
#      `kamailio -c` misses, e.g. force_send_socket on a non-listened port).
#   4. Wait until active dialogs <= MAX_ACTIVE (dialog db_mode=0: state of calls
#      in progress is lost on restart; media keeps flowing via RTPEngine).
#   5. Backup, install, restart.
#   6. Verify: unit active, SIP OPTIONS answered on both sockets, inbound_fs_numbers
#      htable loaded, no crash in the journal, same MainPID after STABLE_SECS.
#   7. Any failed check → restore backup, restart, verify again.
#
# Usage: restart_kamailio_safe.sh [--yes] [--force-now] [--check-only]
#   --yes         no interactive confirmation of the diff
#   --force-now   skip the low-traffic gate
#   --check-only  only steps 1-3 (no restart)
# Env: MAX_ACTIVE (10)  WAIT_MAX_MIN (20)  STABLE_SECS (60)
#===============================================================================
set -uo pipefail

REPO=/opt/ApoloBilling
REPO_CFG="$REPO/config/kamailio/kamailio.cfg"
LIVE_CFG=/etc/kamailio/kamailio.cfg
PING="python3 -I $REPO/scripts/maint/sip_ping.py"
EXT_SIP=10.124.193.41   # carrier-facing listener (EXTERNAL_IP_ADDR)
INT_SIP=10.118.0.6      # FreeSWITCH-facing listener (INTERNAL_IP_ADDR)
INT_PORT=5080

MAX_ACTIVE="${MAX_ACTIVE:-10}"
WAIT_MAX_MIN="${WAIT_MAX_MIN:-20}"
STABLE_SECS="${STABLE_SECS:-60}"
YES=0; FORCE=0; CHECK_ONLY=0
for a in "$@"; do
    case "$a" in
        --yes) YES=1 ;;
        --force-now) FORCE=1 ;;
        --check-only) CHECK_ONLY=1 ;;
        *) echo "unknown option $a"; exit 2 ;;
    esac
done

TS=$(date +%Y%m%d-%H%M%S)
WORK=$(mktemp -d /tmp/kam-maint-$TS.XXXX)
LOG=/var/log/apolo-maint-kamailio-$TS.log
BACKUP="$LIVE_CFG.bak-$TS-maint"
exec > >(tee -a "$LOG") 2>&1

log()  { echo "[$(date +%T)] $*"; }
fail() { log "ERROR: $*"; exit 1; }
[ "$(id -u)" -eq 0 ] || fail "run as root"

#--- 1. candidate ----------------------------------------------------------------
CRED=$(grep -oP 'mysql://[^@/]+@' "$LIVE_CFG" | head -1)
[ -n "$CRED" ] || fail "could not read DB credentials from $LIVE_CFG"
sed "s#mysql://kamailio:YOUR_KAMAILIO_DB_PASSWORD@#$CRED#g" "$REPO_CFG" > "$WORK/kamailio.cfg"
grep -q YOUR_KAMAILIO_DB_PASSWORD "$WORK/kamailio.cfg" && fail "placeholder left in candidate"

#--- 2. diff ---------------------------------------------------------------------
log "Diff live → candidate (ignoring trailing spaces):"
diff -u <(sed 's/[[:space:]]*$//' "$LIVE_CFG") <(sed 's/[[:space:]]*$//' "$WORK/kamailio.cfg") \
    | sed -E 's#mysql://[^@]+@#mysql://***@#g' | tail -n +3
if cmp -s "$LIVE_CFG" "$WORK/kamailio.cfg"; then
    log "Candidate identical to live config; nothing to install."
    [ "$CHECK_ONLY" -eq 1 ] && exit 0
fi
if [ "$YES" -eq 0 ] && [ "$CHECK_ONLY" -eq 0 ]; then
    read -r -p "Install this change and restart Kamailio? [y/N] " ans
    [ "$ans" = "y" ] || fail "aborted by operator"
fi

#--- 3. validation ---------------------------------------------------------------
log "kamailio -c"
kamailio -c -f "$WORK/kamailio.cfg" >"$WORK/check.out" 2>&1 || { tail -20 "$WORK/check.out"; fail "kamailio -c failed"; }

log "Test-start of a scratch copy (expected to stop only at 'Address already in use')"
cp "$WORK/kamailio.cfg" "$WORK/scratch.cfg"
sed -i "s#/var/run/kamailio/kamailio_rpc.fifo#$WORK/rpc.fifo#; s#unix:/var/run/kamailio/kamailio_ctl#unix:$WORK/ctl#" "$WORK/scratch.cfg"
for f in /etc/kamailio/*.cfg; do
    b=$(basename "$f"); [ "$b" != kamailio.cfg ] && ln -sf "$f" "$WORK/$b"
done
(cd "$WORK" && timeout 30 kamailio -DD -E -P "$WORK/k.pid" -f "$WORK/scratch.cfg" >"$WORK/start.out" 2>&1)
pkill -f "$WORK/scratch.cfg" 2>/dev/null
grep -q "Address already in use" "$WORK/start.out" || { tail -30 "$WORK/start.out"; fail "scratch start did not reach socket bind"; }
BAD=$(grep -E "ERROR|CRITICAL|ALERT" "$WORK/start.out" | grep -v "Address already in use" | grep -vE "bind\(|tcp_init|udp_init|init_sock")
[ -z "$BAD" ] || { echo "$BAD" | head -20; fail "scratch start reported errors"; }
log "Validation OK"
[ "$CHECK_ONLY" -eq 1 ] && { log "--check-only: done, nothing changed"; exit 0; }

#--- 4. low-traffic gate ---------------------------------------------------------
active_dlg() { kamcmd dlg.stats_active 2>/dev/null | awk '/all:/{print $2}'; }
if [ "$FORCE" -eq 0 ]; then
    deadline=$(( $(date +%s) + WAIT_MAX_MIN*60 ))
    while :; do
        n=$(active_dlg); n=${n:-999}
        [ "$n" -le "$MAX_ACTIVE" ] && { log "Active dialogs: $n (<= $MAX_ACTIVE) → go"; break; }
        [ "$(date +%s)" -ge "$deadline" ] && fail "still $n active dialogs after $WAIT_MAX_MIN min; nothing changed"
        log "Active dialogs: $n > $MAX_ACTIVE, waiting 30s..."
        sleep 30
    done
fi

#--- 5-6. install, restart, verify -----------------------------------------------
verify() {
    local since=$1 pid0 pid1 cnt
    for i in $(seq 1 15); do systemctl is-active -q kamailio && break; sleep 1; done
    systemctl is-active -q kamailio || { log "unit not active"; return 1; }
    sleep 2
    $PING $EXT_SIP 5060 $EXT_SIP >/dev/null || { log "no OPTIONS reply on $EXT_SIP:5060"; return 1; }
    $PING $INT_SIP $INT_PORT $INT_SIP >/dev/null || { log "no OPTIONS reply on $INT_SIP:$INT_PORT"; return 1; }
    cnt=$(kamcmd htable.dump inbound_fs_numbers 2>/dev/null | grep -c 'name:')
    log "htable inbound_fs_numbers: $cnt entries"
    [ "$cnt" -gt 0 ] || { log "htable inbound_fs_numbers empty"; return 1; }
    pid0=$(systemctl show -p MainPID --value kamailio)
    log "Stability window ${STABLE_SECS}s (MainPID $pid0)..."
    sleep "$STABLE_SECS"
    pid1=$(systemctl show -p MainPID --value kamailio)
    [ "$pid0" = "$pid1" ] && systemctl is-active -q kamailio || { log "Kamailio restarted during the window ($pid0 → $pid1)"; return 1; }
    if tail -c +"$((since + 1))" /var/log/kamailio.log 2>/dev/null | grep -qaE "exited by a signal|qm_free\(\): BUG|core dumped"; then
        log "crash signature in /var/log/kamailio.log"; return 1
    fi
    $PING $EXT_SIP 5060 $EXT_SIP >/dev/null || { log "no OPTIONS reply after the window"; return 1; }
    return 0
}

cp -a "$LIVE_CFG" "$BACKUP" || fail "backup failed"
log "Backup: $BACKUP"
install -m 0644 -o root -g root "$WORK/kamailio.cfg" "$LIVE_CFG"
# byte offset of kamailio.log: only lines written after the restart are checked
SINCE=$(stat -c %s /var/log/kamailio.log 2>/dev/null || echo 0)
log "systemctl restart kamailio"
systemctl restart kamailio
if verify "$SINCE"; then
    log "OK: Kamailio running the new config. Log: $LOG"
    kamcmd dlg.stats_active 2>/dev/null
    exit 0
fi

#--- 7. rollback -----------------------------------------------------------------
log "VERIFICATION FAILED → rolling back to $BACKUP"
cp -a "$BACKUP" "$LIVE_CFG"
# byte offset of kamailio.log: only lines written after the restart are checked
SINCE=$(stat -c %s /var/log/kamailio.log 2>/dev/null || echo 0)
systemctl restart kamailio
if STABLE_SECS=20 verify "$SINCE"; then
    log "Rollback OK: Kamailio running the previous config. Log: $LOG"
else
    log "ROLLBACK ALSO FAILED — intervene manually: journalctl -u kamailio -n 100"
fi
exit 1
