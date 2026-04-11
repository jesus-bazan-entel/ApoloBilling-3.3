-- Add trunk destination support for inbound routes
-- When destination_trunk_id is set, route via gateway instead of IP:port

ALTER TABLE routing_inbound_routes
ADD COLUMN IF NOT EXISTS destination_trunk_id UUID REFERENCES routing_trunks(id) ON DELETE SET NULL;

COMMENT ON COLUMN routing_inbound_routes.destination_trunk_id IS 'If set, route via this trunk gateway instead of destination_host:port';
