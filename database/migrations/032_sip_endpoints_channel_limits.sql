-- Migration 032: Channel limits by endpoint type
-- Troncal (PBX): 1-150 canales concurrentes.
-- Extension: 1-20 registros simultaneos (limite historico).
-- El enforcement en tiempo real lo hara Kamailio (call_settings/concurrent_calls);
-- este CHECK es la guarda de integridad en la capa de datos.

BEGIN;

ALTER TABLE sip_endpoints
    DROP CONSTRAINT IF EXISTS chk_sip_endpoints_channel_limits;

ALTER TABLE sip_endpoints
    ADD CONSTRAINT chk_sip_endpoints_channel_limits CHECK (
        (endpoint_type = 'trunk' AND max_registrations BETWEEN 1 AND 150)
        OR
        (endpoint_type = 'extension' AND max_registrations BETWEEN 1 AND 20)
    );

COMMENT ON CONSTRAINT chk_sip_endpoints_channel_limits ON sip_endpoints IS
'Trunk: 1-150 canales concurrentes. Extension: 1-20 registros simultaneos.';

COMMIT;
