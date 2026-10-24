#!/bin/bash
#===============================================================================
# purge_kamailio_acc.sh
#
# Kamailio's own accounting table (kamailio.acc) logs every call independently
# of ApoloBilling's real CDR pipeline (PostgreSQL `cdrs`, written by
# rust-billing-engine). Nothing in ApoloBilling reads kamailio.acc/kamailio.cdrs
# — it's Kamailio's internal audit log — so it grows unbounded unless purged.
#
# Deletes rows older than RETENTION_DAYS in small batches (keyed off the PK,
# not the unindexed `time` column) so the live INSERT traffic from Kamailio
# is never blocked for more than a few ms at a time.
#
# Added 2026-08-09 after analysis found kamailio.acc at 1.86M rows / 412MB
# with no retention policy, contributing to MariaDB buffer pool / disk pressure.
#
# Also purges kamailio.acc_cdrs (dialog CDRs, see kamailio_cdr_sync.rs).
#
# Usage: purge_kamailio_acc.sh [retention_days]   (default 90)
#===============================================================================
set -euo pipefail

RETENTION_DAYS="${1:-90}"
BATCH=5000
SLEEP_BETWEEN_BATCHES=0.15
ENV_FILE="/opt/ApoloBilling/rust-backend/.env"
LOG_FILE="/var/log/apolo-purge-acc.log"

log() { echo "$(date '+%F %T') $*" >> "$LOG_FILE"; }

if [[ ! -f "$ENV_FILE" ]]; then
    echo "ERROR: $ENV_FILE not found" >&2
    exit 1
fi

DB_URL=$(grep -E '^KAMAILIO_DATABASE_URL=' "$ENV_FILE" | head -1 | cut -d'=' -f2-)
if [[ -z "$DB_URL" ]]; then
    log "KAMAILIO_DATABASE_URL not set — skipping (no MySQL/Kamailio DB configured)"
    exit 0
fi

# mysql://user:pass@host:port/db
DB_USER=$(sed -E 's#mysql://([^:]+):.*#\1#' <<<"$DB_URL")
DB_PASS=$(sed -E 's#mysql://[^:]+:([^@]+)@.*#\1#' <<<"$DB_URL")
DB_HOST=$(sed -E 's#.*@([^:/]+).*#\1#' <<<"$DB_URL")
DB_NAME=$(sed -E 's#.*/([^/?]+)$#\1#' <<<"$DB_URL")

MYSQL=(mysql -u"$DB_USER" -p"$DB_PASS" -h"$DB_HOST" -N)

# purge_table <table> <time column>
purge_table() {
    local table="$1" time_col="$2"
    local boundary total affected start end
    boundary=$("${MYSQL[@]}" -e \
        "SELECT COALESCE(MAX(id),0) FROM \`$DB_NAME\`.$table WHERE $time_col < DATE_SUB(NOW(), INTERVAL $RETENTION_DAYS DAY);")

    if [[ "$boundary" -eq 0 ]]; then
        log "nothing to purge from $table (retention=${RETENTION_DAYS}d)"
        return 0
    fi

    total=0
    start=$(date +%s)
    while true; do
        affected=$("${MYSQL[@]}" -e "
            DELETE FROM \`$DB_NAME\`.$table WHERE id <= $boundary ORDER BY id LIMIT $BATCH;
            SELECT ROW_COUNT();
        ")
        total=$((total + affected))
        [[ "$affected" -eq 0 ]] && break
        sleep "$SLEEP_BETWEEN_BATCHES"
    done
    end=$(date +%s)

    log "purged $total rows from $table (retention=${RETENTION_DAYS}d, boundary_id=$boundary) in $((end-start))s"
}

purge_table acc time
# Dialog CDRs (2026-10-24): rust-billing-engine imports via=direct rows within
# seconds, so retention only bounds the via=fs cross-check rows.
purge_table acc_cdrs start_time
