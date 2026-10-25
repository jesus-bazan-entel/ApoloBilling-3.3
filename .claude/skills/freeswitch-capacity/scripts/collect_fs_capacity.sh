#!/bin/bash
#===============================================================================
# collect_fs_capacity.sh — read-only snapshot of a FreeSWITCH node's capacity
#
# Gathers configuration, live load, host health and the usual traps in one
# report, so the analysis starts from facts instead of guesses. Changes nothing:
# only fs_cli "show/status" commands, file reads and short host samples.
#
# Usage: collect_fs_capacity.sh [-c /etc/freeswitch] [-s 10] [-o report.txt]
#   -c  FreeSWITCH config dir (default /etc/freeswitch)
#   -s  seconds to sample CPU / disk (default 10)
#   -o  also write the report to this file
# Run as root (or a user allowed to use fs_cli and read the config).
#===============================================================================
set -uo pipefail
export LC_ALL=C   # pidstat/vmstat/du print localized labels and decimal commas otherwise

CONF=/etc/freeswitch
SAMPLE=10
OUT=""
while getopts "c:s:o:" opt; do
    case $opt in
        c) CONF=$OPTARG ;;
        s) SAMPLE=$OPTARG ;;
        o) OUT=$OPTARG ;;
        *) echo "usage: $0 [-c confdir] [-s seconds] [-o file]" >&2; exit 2 ;;
    esac
done
[ -n "$OUT" ] && exec > >(tee "$OUT") 2>&1

fscli() { timeout 10 fs_cli -x "$1" 2>/dev/null; }
section() { printf '\n===== %s =====\n' "$1"; }
# value of <param name="X" value="Y"/> that is not inside an XML comment
param() {
    python3 -I - "$1" "$2" <<'EOF' 2>/dev/null
import sys, xml.etree.ElementTree as E
try:
    root = E.parse(sys.argv[1]).getroot()
except Exception:
    sys.exit(0)
for p in root.iter('param'):
    if p.get('name') == sys.argv[2]:
        print(p.get('value')); break
EOF
}

section "NODE"
echo "host=$(hostname) date=$(date '+%F %T %Z')"
echo "virtualization=$(systemd-detect-virt 2>/dev/null || echo unknown)"
echo "cpus=$(nproc) mem_total_mb=$(awk '/MemTotal/{print int($2/1024)}' /proc/meminfo) mem_avail_mb=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)"
echo "load=$(cut -d' ' -f1-3 /proc/loadavg)"

section "FREESWITCH STATUS"
if ! fscli status | grep -q "is ready"; then
    echo "fs_cli not answering: is FreeSWITCH running and the ESL password right?"
fi
fscli version
fscli status
echo "calls=$(fscli 'show calls count' | awk '/total/{print $1}')"
echo "channels=$(fscli 'show channels count' | awk '/total/{print $1}')"
echo "registrations=$(fscli 'show registrations count' | awk '/total/{print $1}')"
PID=$(pgrep -xo freeswitch)
if [ -n "$PID" ]; then
    echo "pid=$PID threads=$(ls /proc/$PID/task | wc -l) rss_mb=$(( $(awk '/VmRSS/{print $2}' /proc/$PID/status) / 1024 ))"
    echo "started=$(ps -o lstart= -p $PID)"
    systemctl show -p NRestarts,ActiveEnterTimestamp freeswitch 2>/dev/null
fi

section "PROCESS CPU (${SAMPLE}s sample, % of ONE core)"
if [ -n "${PID:-}" ] && command -v pidstat >/dev/null; then
    pidstat -u -p "$PID" "$SAMPLE" 1 | awk '/Average/ && $0 !~ /%usr/ {print "fs_cpu_pct_one_core=" $(NF-2)}'
else
    echo "pidstat not available"
fi

section "HOST CPU / DISK (${SAMPLE}s vmstat)"
vmstat 1 "$SAMPLE" | awk 'NR==2{print "      " $0} NR>2{bi+=$9; bo+=$10; wa+=$16; st+=$17; id+=$15; n++} END{if(n) printf "avg: idle=%.0f%% iowait=%.0f%% steal=%.0f%% disk_read_kBps=%.0f disk_write_kBps=%.0f\n", id/n, wa/n, st/n, bi/n, bo/n}'

section "CORE LIMITS (switch.conf.xml)"
SW=$CONF/autoload_configs/switch.conf.xml
for p in max-sessions sessions-per-second rtp-start-port rtp-end-port loglevel core-db-name core-db-dsn \
         max-db-handles min-idle-cpu enable-clock-nanosleep enable-monotonic-timing; do
    printf '%-24s %s\n' "$p" "$(param "$SW" "$p")"
done
echo "core_db_files: $(ls -la /dev/shm/core.db /var/lib/freeswitch/db/core.db 2>/dev/null | awk '{print $NF" "$5"B"}' | paste -sd';')"
want=$(param "$SW" core-db-name)
if [ -n "$want" ] && [ ! -f "$want" ]; then
    echo "core_db_note: core-db-name=$want is configured but not in use yet (takes effect at the next FreeSWITCH restart)"
fi

section "SYSTEMD LIMITS"
systemctl show freeswitch -p LimitNOFILE -p LimitNPROC -p LimitSTACK -p CPUSchedulingPolicy -p IOSchedulingClass 2>/dev/null

section "SIP PROFILES"
fscli 'sofia status'
for f in "$CONF"/sip_profiles/*.xml; do
    [ -f "$f" ] || continue
    name=$(basename "$f" .xml)
    printf -- '--- %s\n' "$name"
    for p in sip-port rtp-ip rtp-timer-name inbound-bypass-media inbound-proxy-media bypass-media-after-bridge \
             inbound-late-negotiation rtp-timeout-sec rtp-hold-timeout-sec enable-timer session-timeout \
             inbound-codec-prefs outbound-codec-prefs; do
        v=$(param "$f" "$p"); [ -n "$v" ] && printf '  %-28s %s\n' "$p" "$v"
    done
done

section "MEDIA IN USE (live channels)"
fscli 'show channels as json' | python3 -I -c '
import sys, json, collections
try:
    rows = json.load(sys.stdin).get("rows") or []
except Exception:
    rows = []
c = collections.Counter()
transcoding = 0
for r in rows:
    rc, wc = r.get("read_codec") or "-", r.get("write_codec") or "-"
    c[(rc, wc)] += 1
print("channels_sampled=%d" % len(rows))
for (rc, wc), n in c.most_common(8):
    print("  read=%-8s write=%-8s %d" % (rc, wc, n))
codecs = {r.get("read_codec") for r in rows if r.get("read_codec")}
print("distinct_codecs=%s" % ",".join(sorted(codecs)))
print("transcoding_hint=%s" % ("yes (more than one codec across legs: check bridged pairs)" if len(codecs) > 1 else "no (single codec)"))
' 2>/dev/null

section "DTMF PAYLOAD TYPE / PTIME BY LEG (first 20 channels)"
fscli 'show channels as json' | python3 -I -c '
import sys, json
try:
    rows = json.load(sys.stdin).get("rows") or []
except Exception:
    rows = []
for r in rows[:20]:
    print(r.get("uuid"), r.get("direction"))
' 2>/dev/null | while read -r u d; do
    fscli "uuid_dump $u" | awk -F': ' -v d="$d" '
        /^variable_sip_profile_name:/{prof=$2} /^variable_remote_media_ip:/{ip=$2}
        /^variable_rtp_2833_send_payload:/{te=$2} /^variable_rtp_use_codec_ptime:/{pt=$2}
        END{print d, prof, "remote_media=" ip, "telephone-event_pt=" te, "ptime=" pt}'
done | sort | uniq -c | sort -rn

section "MODULES"
fscli 'show modules' | awk -F, 'NR>1 && $3 ~ /mod_/ {print $3}' | sort -u | paste -sd' '
echo "timer_modules: $(fscli 'show timer' | awk -F, 'NR>1 && NF>1 {print $2}' | paste -sd' ')"

section "DISK WRITERS PER CALL"
for d in /var/log/freeswitch/cdr-csv /var/log/freeswitch/xml_cdr /var/log/freeswitch/json_cdr; do
    [ -d "$d" ] && echo "$d $(du -sh "$d" 2>/dev/null | cut -f1) newest=$(ls -t "$d" 2>/dev/null | head -1)"
done
ls -la /var/log/freeswitch/freeswitch.log 2>/dev/null | awk '{print "freeswitch.log size=" $5}'

section "RECENT PROBLEMS (journal, last 24h)"
journalctl -u freeswitch --since "-24h" --no-pager 2>/dev/null | grep -ciE "segv|core dumped|exited" | sed 's/^/crash_lines=/'
grep -acE "Throttle Error|Max Sessions|MAX_SESSIONS|Too many|CRIT" /var/log/freeswitch/freeswitch.log 2>/dev/null | sed 's/^/limit_or_crit_log_lines=/'

section "NEIGHBOURS (optional)"
command -v kamcmd >/dev/null && echo "kamailio_dialogs: $(kamcmd dlg.stats_active 2>/dev/null | awk '/all:/{print $2}')"
[ -d /proc/rtpengine ] && echo "rtpengine_kernel_table: present ($(grep -c . /proc/rtpengine/0/list 2>/dev/null) lines in /proc/rtpengine/0/list)"
lsmod 2>/dev/null | grep -q xt_RTPENGINE && echo "xt_RTPENGINE: loaded"

echo
echo "done (read-only)."
