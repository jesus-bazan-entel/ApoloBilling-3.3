#!/bin/bash
# 2026-11-02 03:30 window: installs config/kamailio/kamailio.cfg (keepalive trunk
# health: ka_* htables) with restart_kamailio_safe.sh (low-traffic gate, checks,
# auto-rollback), then records what the collector will read.
set -uo pipefail
LOG=/var/log/apolo-maint-keepalive-$(date +%Y%m%d-%H%M%S).log
exec > >(tee -a "$LOG") 2>&1
echo "[$(date +%T)] start"
MAX_ACTIVE=10 WAIT_MAX_MIN=60 /bin/bash /opt/ApoloBilling/scripts/maint/restart_kamailio_safe.sh --yes
rc=$?
echo "[$(date +%T)] restart_kamailio_safe.sh rc=$rc"
sleep 90
echo "--- keepalive.list"; kamcmd keepalive.list
echo "--- ka_status"; kamcmd htable.dump ka_status | grep -E "name|value"
echo "--- ka_ok"; kamcmd htable.dump ka_ok | grep -E "name|value"
echo "[$(date +%T)] end"
exit $rc
