-- Kamailio (MySQL/MariaDB): per-request DB lookups out of the SIP workers.
-- Date: 2026-11-01 (VoIP audit). Applied by scripts/install_server.sh. Idempotent.

-- View loaded by htable `src_gw` (kamailio.cfg, reloaded every 30s): source IP →
-- "type,gwid". Replaces the blocking `select type, gwid from dr_gateways where
-- address like '$si%'` that SET_CALLSRC_INFO ran on every initial request.
-- Exact IPv4 match ('sip:' prefix and port stripped); when several gateways
-- share an IP, the one with a gwgroup wins, then the lowest gwid - the same row
-- the old query returned in practice, now deterministic.
CREATE OR REPLACE SQL SECURITY INVOKER VIEW apolo_src_gw AS
SELECT ip, CONCAT(type, ',', gwid) AS info FROM (
  SELECT s.ip, s.type, s.gwid,
         ROW_NUMBER() OVER (PARTITION BY s.ip ORDER BY (m.gwgroupid IS NULL), s.gwid) AS rn
  FROM (SELECT gwid, type,
               SUBSTRING_INDEX(REGEXP_REPLACE(address, '^sips?:', ''), ':', 1) AS ip
        FROM dr_gateways) s
  LEFT JOIN dsip_gw2gwgroup m ON m.gwid = CAST(s.gwid AS CHAR)
  WHERE s.ip REGEXP '^[0-9]{1,3}([.][0-9]{1,3}){3}$'
) t WHERE rn = 1;

-- purge_kamailio_acc.sh looks up the retention boundary by time: without this
-- index it full-scanned acc (2.3M rows, 3.7-14 s) every night.
-- Online (no write lock); on a busy node run at low traffic.
SET SESSION lock_wait_timeout = 5;
ALTER TABLE acc ADD INDEX IF NOT EXISTS acc_time_idx (time), ALGORITHM=INPLACE, LOCK=NONE;
