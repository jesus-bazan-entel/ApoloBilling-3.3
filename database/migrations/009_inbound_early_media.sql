-- Add early media support for inbound routes
-- When enabled, sends 183 Session Progress with ringback tone to carrier

ALTER TABLE routing_inbound_routes 
ADD COLUMN send_early_media BOOLEAN NOT NULL DEFAULT false;

COMMENT ON COLUMN routing_inbound_routes.send_early_media IS 
'When true, sends 183 Session Progress with ringback tone before bridging';
