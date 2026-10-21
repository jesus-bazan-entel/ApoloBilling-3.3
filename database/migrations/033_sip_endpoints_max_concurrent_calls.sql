-- Migration 033: Per-endpoint concurrent-call limit.
-- auth_mode=credentials endpoints (extensions and credential-auth trunks)
-- bypass Kamailio routing (RELAY_LANEXT), so their concurrent-call limit
-- lives here and is enforced by the billing engine on FreeSWITCH
-- CHANNEL_CREATE (active_calls count). auth_mode=ip trunks are enforced in
-- Kamailio (call_settings, dSIPRouter tables) using this same column, synced
-- via sync_trunk_call_settings.
-- Extension: 1-20 llamadas simultaneas. Trunk: 1-150 canales concurrentes.

BEGIN;

ALTER TABLE sip_endpoints
    ADD COLUMN IF NOT EXISTS max_concurrent_calls INTEGER NOT NULL DEFAULT 2;

-- Backfill from the existing knob so current effective behavior is preserved.
UPDATE sip_endpoints
SET max_concurrent_calls = GREATEST(1, LEAST(COALESCE(max_registrations, 2), 20))
WHERE endpoint_type = 'extension';

UPDATE sip_endpoints
SET max_concurrent_calls = GREATEST(1, LEAST(COALESCE(max_registrations, 2), 150))
WHERE endpoint_type = 'trunk';

ALTER TABLE sip_endpoints
    DROP CONSTRAINT IF EXISTS chk_sip_endpoints_call_limits;

ALTER TABLE sip_endpoints
    ADD CONSTRAINT chk_sip_endpoints_call_limits CHECK (
        (endpoint_type = 'trunk' AND max_concurrent_calls BETWEEN 1 AND 150)
        OR
        (endpoint_type = 'extension' AND max_concurrent_calls BETWEEN 1 AND 20)
    );

COMMENT ON CONSTRAINT chk_sip_endpoints_call_limits ON sip_endpoints IS
'Extension y troncal credentials: 1-20/1-150 via billing engine (ESL). Troncal ip: 1-150 via Kamailio call_settings.';

COMMIT;
