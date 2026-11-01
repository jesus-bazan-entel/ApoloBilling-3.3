#!/bin/bash
#===============================================================================
# disk_guard.sh — alert when the root filesystem fills up (never deletes).
#
# MariaDB (Kamailio acc/acc_cdrs), PostgreSQL, logs and captures share "/".
# A full disk makes InnoDB suspend writes, and every Kamailio worker waiting on
# MySQL stalls SIP signalling. On 2026-11-01 "/" was at 84% because
# /var/log/daemon.log (not in logrotate) had grown to 23 GB.
#
# Logs to syslog (tag apolo-disk-guard): warning at WARN_PCT, crit at CRIT_PCT,
# with the biggest directories under /var/log and /var/lib to speed up triage.
# Cron: /etc/cron.d/apolo-disk-guard (every 10 min, installed by install_server.sh)
# Env: WARN_PCT (85)  CRIT_PCT (92)  FS (/)
#===============================================================================
set -uo pipefail

WARN_PCT="${WARN_PCT:-85}"
CRIT_PCT="${CRIT_PCT:-92}"
FS="${FS:-/}"

pct=$(df --output=pcent "$FS" | tail -1 | tr -dc '0-9')
[ -n "$pct" ] || exit 0
[ "$pct" -lt "$WARN_PCT" ] && exit 0

prio=warning
[ "$pct" -ge "$CRIT_PCT" ] && prio=crit
top=$(du -xhs /var/log/* /var/lib/* 2>/dev/null | sort -rh | head -5 | awk '{printf "%s=%s ", $2, $1}')
logger -t apolo-disk-guard -p "user.$prio" \
    "$FS at ${pct}% (warn ${WARN_PCT}%, crit ${CRIT_PCT}%) - MariaDB/Kamailio stall risk. Top: $top"
