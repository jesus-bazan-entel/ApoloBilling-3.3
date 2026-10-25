#!/bin/bash
#===============================================================================
# run_window_bc.sh
#
# Unattended maintenance window, phases B and C of the PRD
# "Ventana de mantenimiento: reinicio seguro de Kamailio y FreeSWITCH":
#   B. Kamailio: install the repo kamailio.cfg (direct-path PAI fix) with
#      restart_kamailio_safe.sh.
#   C. FreeSWITCH: restart_freeswitch_safe.sh (RTP range 20002-32768, loglevel
#      info). No --timerfd: on 2026-10-25 it raised calls with >1% RTP loss from
#      3% to 30% and was rolled back.
#
# PRD rules enforced here:
#   - Kamailio goes only if its pending diff is exactly the PAI fix (guard).
#   - FreeSWITCH is not touched if Kamailio was skipped, failed or rolled back.
#   - 5 minutes between phases so a problem can be attributed.
# Each phase keeps its own low-traffic gate (<= 10), verification and rollback.
#
# Usage: run_window_bc.sh            (meant for systemd-run --on-calendar)
#===============================================================================
set -uo pipefail

MAINT=/opt/ApoloBilling/scripts/maint
TS=$(date +%Y%m%d-%H%M%S)
LOG=/var/log/apolo-maint-window-$TS.log
PAUSE_SECS="${PAUSE_SECS:-300}"
exec > >(tee -a "$LOG") 2>&1

log()    { echo "[$(date +%T)] $*"; }
finish() { log "RESULT: $1"; logger -t apolo-maint "window B+C: $1 (log $LOG)"; exit "$2"; }

log "Window B+C start. Calls: $(fs_cli -x 'show calls count' 2>/dev/null | grep total) | dialogs: $(kamcmd dlg.stats_active 2>/dev/null | awk '/all:/{print $2}')"

#--- Guard: the Kamailio change must be exactly the direct-path PAI fix ----------
CHECK=$(mktemp)
"$MAINT/restart_kamailio_safe.sh" --check-only >"$CHECK" 2>&1
rc=$?
cat "$CHECK"
added=$(grep -cE '^\+($|[^+])' "$CHECK")
removed=$(grep -cE '^-($|[^-])' "$CHECK")
if [ $rc -ne 0 ] || ! grep -q 'Validation OK' "$CHECK"; then
    finish "ABORTED before Kamailio: validation failed, nothing changed" 1
fi
if grep -q 'Candidate identical to live config' "$CHECK"; then
    log "Kamailio already runs the repo config (PAI fix live): skipping phase B"
elif [ "$added" -ne 10 ] || [ "$removed" -ne 0 ] || ! grep -qF '$dlg_var(did) = $tU;' "$CHECK"; then
    finish "ABORTED before Kamailio: unexpected diff (+$added/-$removed), review the repo config; nothing changed" 1
else
    #--- Phase B -------------------------------------------------------------
    log "Phase B: Kamailio"
    if ! "$MAINT/restart_kamailio_safe.sh" --yes; then
        finish "Phase B FAILED (aborted or rolled back); FreeSWITCH untouched per PRD" 1
    fi
    log "Phase B OK. Pause ${PAUSE_SECS}s before phase C"
    sleep "$PAUSE_SECS"
fi
rm -f "$CHECK"

#--- Phase C -----------------------------------------------------------------
log "Phase C: FreeSWITCH"
if ! "$MAINT/restart_freeswitch_safe.sh" --yes; then
    finish "Phase C FAILED (aborted or rolled back); Kamailio phase B result stands" 1
fi

finish "OK: phases B and C applied. Pending manual checks: PRD phase D (audio, CDRs, registrations, PAI)" 0
