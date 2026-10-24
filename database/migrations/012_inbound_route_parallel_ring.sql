-- Migration: Add parallel-ring (fork to registered push device) to inbound routes
-- Date: 2026-06-01
-- Description: Adds ring_registered_parallel to routing_inbound_routes. When true,
--              the generated from-kamailio dialplan appends a simultaneous bridge
--              leg to the registered device's current contact (${sofia_contact(...)}),
--              so an inbound call rings the primary destination AND the registered
--              mobile/push device at the same time (first to answer wins). Mirrors
--              009_inbound_early_media.sql.

ALTER TABLE routing_inbound_routes
ADD COLUMN IF NOT EXISTS ring_registered_parallel BOOLEAN NOT NULL DEFAULT false;

COMMENT ON COLUMN routing_inbound_routes.ring_registered_parallel IS 'If true, also ring the registered SIP device (sofia_contact) for the destination number in parallel with the primary destination';
