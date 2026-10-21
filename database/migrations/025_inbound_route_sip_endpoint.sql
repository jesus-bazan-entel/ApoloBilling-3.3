-- 025: Rutas entrantes pueden apuntar a un dispositivo SIP tipo Troncal (PBX)
-- `sip_endpoints.endpoint_type = 'trunk'` (ej: PBX GC GROUP) además de los
-- `routing_trunks` legacy. Solo puede haber una referencia a la vez.

ALTER TABLE inbound_routes
    ADD COLUMN IF NOT EXISTS sip_endpoint_id UUID REFERENCES sip_endpoints(id) ON DELETE SET NULL;

ALTER TABLE inbound_routes DROP CONSTRAINT IF EXISTS chk_inbound_route_single_dest;
ALTER TABLE inbound_routes
    ADD CONSTRAINT chk_inbound_route_single_dest
    CHECK (gateway_id IS NULL OR sip_endpoint_id IS NULL);

-- Backfill: solo rutas gateway que NO tienen gateway_id (fallback por IP cruda),
-- cruzando destination_host con el host del endpoint trunk.
-- Ej: ENTEL-GC-GROUP (10.40.41.124) -> PBX GC GROUP
UPDATE inbound_routes r
SET sip_endpoint_id = sub.endpoint_id
FROM (
    SELECT DISTINCT ON (r2.id)
        r2.id AS route_id,
        se.id  AS endpoint_id
    FROM inbound_routes r2
    JOIN sip_endpoints se ON se.host = r2.destination_host
    WHERE r2.gateway_id IS NULL
      AND r2.sip_endpoint_id IS NULL
      AND r2.destination_type = 'gateway'
      AND se.endpoint_type = 'trunk'
      AND se.enabled = true
      AND se.host IS NOT NULL
    ORDER BY r2.id, se.is_default DESC, se.name
) sub
WHERE r.id = sub.route_id;
