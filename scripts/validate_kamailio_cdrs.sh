#!/bin/bash
#===============================================================================
# validate_kamailio_cdrs.sh
#
# Cross-checks Kamailio dialog CDRs (kamailio.acc_cdrs, via=fs) against the
# FreeSWITCH/ESL CDRs in PostgreSQL for the SAME calls, before trusting
# Kamailio as the only CDR source for direct-routed inbound DIDs.
#
# Calls are matched on caller + called number + start time (±3s), since the
# FS CDR is keyed by the FS channel UUID, not the SIP Call-ID.
#
# Usage: validate_kamailio_cdrs.sh [hours]   (default 2)
#===============================================================================
set -euo pipefail

HOURS="${1:-2}"
ENV_FILE="/opt/ApoloBilling/rust-backend/.env"

PG_URL=$(grep -E '^DATABASE_URL=' "$ENV_FILE" | head -1 | cut -d'=' -f2-)
MY_URL=$(grep -E '^KAMAILIO_DATABASE_URL=' "$ENV_FILE" | head -1 | cut -d'=' -f2-)
MY_USER=$(sed -E 's#mysql://([^:]+):.*#\1#' <<<"$MY_URL")
MY_PASS=$(sed -E 's#mysql://[^:]+:([^@]+)@.*#\1#' <<<"$MY_URL")
MY_DB=$(sed -E 's#.*/([^/?]+)$#\1#' <<<"$MY_URL")

# Exclude the last 2 minutes: calls still ringing have no CDR on either side yet.
ACC_TSV=$(mysql -u"$MY_USER" -p"$MY_PASS" "$MY_DB" -N -B -e "
    SELECT src_user, dst_user,
           CAST(UNIX_TIMESTAMP(start_time) AS SIGNED),
           CAST(UNIX_TIMESTAMP(end_time) AS SIGNED),
           IF(answer_ts = '', 0, answer_ts),
           IF(final_ts = '', 0, final_ts),
           sip_code
    FROM acc_cdrs
    WHERE via = 'fs'
      AND start_time BETWEEN NOW() - INTERVAL $HOURS HOUR AND NOW() - INTERVAL 2 MINUTE")

psql "$PG_URL" -q -v ON_ERROR_STOP=1 <<SQL
CREATE TEMP TABLE acc (caller text, called text, start_ts bigint, end_ts bigint,
                       answer_ts bigint, final_ts bigint, sip_code text);
COPY acc FROM STDIN;
${ACC_TSV}
\.
CREATE TEMP TABLE m AS
SELECT a.*, c.call_uuid, c.billsec AS fs_billsec, c.duration AS fs_duration, c.hangup_cause,
       CASE WHEN a.answer_ts > 0 THEN a.end_ts - a.answer_ts ELSE 0 END AS kam_billsec
FROM acc a
LEFT JOIN LATERAL (
    SELECT * FROM cdrs c
    WHERE c.direction = 'inbound' AND c.freeswitch_server_id <> 'kamailio'
      AND c.caller_number = a.caller AND c.called_number = a.called
      AND c.start_time BETWEEN to_timestamp(a.start_ts) - interval '3 s'
                           AND to_timestamp(a.start_ts) + interval '3 s'
    ORDER BY abs(extract(epoch FROM c.start_time) - a.start_ts) LIMIT 1
) c ON true;

\echo '== Matching (Kamailio via=fs vs FreeSWITCH CDR)'
SELECT count(*) AS kamailio_cdrs,
       count(call_uuid) AS matched_fs,
       round(100.0 * count(call_uuid) / nullif(count(*), 0), 2) AS match_pct
FROM m;

\echo '== Answered agreement (both sides say answered / not answered)'
SELECT (answer_ts > 0) AS kam_answered, (fs_billsec > 0) AS fs_answered, count(*)
FROM m WHERE call_uuid IS NOT NULL GROUP BY 1, 2 ORDER BY 1, 2;

\echo '== Billsec delta on answered calls (kamailio - freeswitch, seconds)'
SELECT count(*) AS answered,
       round(avg(kam_billsec - fs_billsec), 2) AS avg_delta,
       min(kam_billsec - fs_billsec) AS min_delta,
       max(kam_billsec - fs_billsec) AS max_delta,
       count(*) FILTER (WHERE abs(kam_billsec - fs_billsec) > 2) AS over_2s
FROM m WHERE call_uuid IS NOT NULL AND answer_ts > 0 AND fs_billsec > 0;

\echo '== Unanswered: Kamailio SIP code vs FreeSWITCH hangup cause'
SELECT sip_code, hangup_cause, count(*) FROM m
WHERE call_uuid IS NOT NULL AND answer_ts = 0
GROUP BY 1, 2 ORDER BY 3 DESC LIMIT 12;

\echo '== FreeSWITCH inbound CDRs in the window with NO Kamailio CDR (should be ~0)'
SELECT count(*) AS fs_without_kamailio FROM cdrs c
WHERE c.direction = 'inbound' AND c.freeswitch_server_id <> 'kamailio'
  AND c.start_time BETWEEN (SELECT to_timestamp(min(start_ts)) FROM acc)
                       AND (SELECT to_timestamp(max(start_ts)) FROM acc)
  AND NOT EXISTS (SELECT 1 FROM acc a WHERE a.caller = c.caller_number AND a.called = c.called_number
                  AND abs(a.start_ts - extract(epoch FROM c.start_time)) <= 3);
SQL
