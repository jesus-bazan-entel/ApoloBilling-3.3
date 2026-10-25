-- 023: routed destination in E.164 + customer on every CDR
--
-- Auditor findings (2026-10-25): cdrs.destination was empty, the dialed number
-- carries Integratel's routing prefix (312217473046) and account_id was NULL
-- on 100% of CDRs. Since this migration the engine writes:
--   * destination_prefix (API/CSV "destination"): the number the call was
--     routed to (FreeSWITCH transformed_number, or the dialed number without
--     the `31`+2-digit prefix) in E.164, NULL when it is not a phone number;
--   * account_id: the customer whose sip_devices.sip_username is that
--     destination (inbound) or the caller (outbound); NULL for numbers that
--     belong to no device (FS Legado customers).
-- caller_number / called_number stay as exchanged with the carrier.
--
-- The column keeps its old billing name for compatibility; only its meaning
-- changed. The functions mirror rust-billing-engine/src/services/numbering.rs
-- and are used here to backfill the current month.

COMMENT ON COLUMN cdrs.destination_prefix IS
    'Routed destination in E.164 (API field "destination"); NULL if not a phone number';
COMMENT ON COLUMN cdrs.account_id IS
    'Customer whose SIP device is the routed destination (inbound) or the caller (outbound)';

-- Number the call was delivered to (Integratel DIDs: 31 + 2-digit code + national)
CREATE OR REPLACE FUNCTION apolo_routed_number(callee text) RETURNS text
LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE WHEN callee ~ '^31[0-9]{10}$' THEN substr(callee, 5) ELSE callee END
$$;

-- Peruvian / international number in E.164, NULL when unrecognisable
CREATE OR REPLACE FUNCTION apolo_e164(n text) RETURNS text
LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE
        WHEN n ~ '^\+[0-9]{8,15}$'                THEN n
        WHEN n ~ '^00[0-9]{8,15}$'                THEN '+' || substr(n, 3)
        WHEN n ~ '^51(9[0-9]{8}|[14-8][0-9]{7})$' THEN '+' || n
        WHEN n ~ '^0[14-8][0-9]{7}$'              THEN '+51' || substr(n, 2)
        WHEN n ~ '^(9[0-9]{8}|[14-8][0-9]{7})$'   THEN '+51' || n
    END
$$;

-- Backfill the current month (older partitions are left as they were);
-- ~800k rows, longer than the role's statement_timeout
SET statement_timeout = 0;
UPDATE cdrs c
SET destination_prefix = apolo_e164(apolo_routed_number(c.called_number)),
    account_id = COALESCE(c.account_id, (
        SELECT d.account_id FROM sip_devices d
        WHERE d.sip_username IN (
            CASE WHEN lower(c.direction) = 'outbound' THEN c.caller_number
                 ELSE apolo_routed_number(c.called_number) END,
            CASE WHEN lower(c.direction) = 'outbound' AND apolo_e164(c.caller_number) LIKE '+51%'
                 THEN substr(apolo_e164(c.caller_number), 4) END)
        ORDER BY d.enabled DESC, d.id LIMIT 1))
WHERE c.start_time >= '2026-10-01 00:00:00-05'
  AND c.destination_prefix IS NULL;
