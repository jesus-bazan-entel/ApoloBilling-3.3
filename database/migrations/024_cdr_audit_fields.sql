-- 024: CDR fields an auditor needs to read a call without interpretation
--
--   * sip_code: final SIP status FreeSWITCH sent on the a-leg
--     (variable_sip_term_status: 200, 486, 487...). The Q.850 hangup_cause
--     alone is ambiguous: an unanswered NORMAL_CLEARING (16) was shown as
--     "Ocupado en ring paralelo", which was an inference.
--   * caller_e164: the caller in E.164. caller_number stays as the carrier
--     sent it (34641413618, %2B-decoded +1850..., 17486128) for
--     reconciliation; carriers present international CLIs without '+'.
--   * source_ip: who sent the INVITE to FreeSWITCH (variable_sip_network_ip):
--     Kamailio for carrier/Legado traffic, the app's public IP for devices.
-- Written by the engine from 2026-10-25; older rows have sip_code/source_ip
-- NULL (not recorded at the time). caller_e164 is backfilled for the current
-- month, and destination_prefix is recomputed with Lima 7-digit numbers.

ALTER TABLE cdrs ADD COLUMN IF NOT EXISTS sip_code smallint;
ALTER TABLE cdrs ADD COLUMN IF NOT EXISTS caller_e164 varchar(20);
ALTER TABLE cdrs ADD COLUMN IF NOT EXISTS source_ip varchar(45);

COMMENT ON COLUMN cdrs.sip_code IS 'Final SIP status sent on the a-leg (FreeSWITCH sip_term_status)';
COMMENT ON COLUMN cdrs.caller_e164 IS 'Caller in E.164; NULL when caller_number is not a phone number';
COMMENT ON COLUMN cdrs.source_ip IS 'IP that sent the INVITE to FreeSWITCH (sip_network_ip)';

-- Same rules as rust-billing-engine/src/services/numbering.rs::to_e164,
-- plus Lima numbers dialed without the area code (7 digits)
CREATE OR REPLACE FUNCTION apolo_e164(n text) RETURNS text
LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE
        WHEN n ~ '^\+[0-9]{8,15}$'                THEN n
        WHEN n ~ '^00[0-9]{8,15}$'                THEN '+' || substr(n, 3)
        WHEN n ~ '^51(9[0-9]{8}|[14-8][0-9]{7})$' THEN '+' || n
        WHEN n ~ '^0[14-8][0-9]{7}$'              THEN '+51' || substr(n, 2)
        WHEN n ~ '^(9[0-9]{8}|[14-8][0-9]{7})$'   THEN '+51' || n
        WHEN n ~ '^[2-9][0-9]{6}$'                THEN '+511' || n
    END
$$;

-- Callers: anything 10-15 digits that is not Peruvian is an international
-- CLI presented without '+' (numbering.rs::caller_to_e164)
CREATE OR REPLACE FUNCTION apolo_caller_e164(n text) RETURNS text
LANGUAGE sql IMMUTABLE AS $$
    SELECT COALESCE(apolo_e164(n),
                    CASE WHEN n ~ '^[1-9][0-9]{9,14}$' THEN '+' || n END)
$$;

SET statement_timeout = 0;

UPDATE cdrs
SET caller_e164 = apolo_caller_e164(caller_number),
    destination_prefix = COALESCE(destination_prefix,
                                  apolo_e164(apolo_routed_number(called_number)))
WHERE start_time >= '2026-10-01 00:00:00-05'
  AND caller_e164 IS NULL;
