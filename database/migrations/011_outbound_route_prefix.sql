-- Migration: Add number transformation fields to outbound routes
-- Date: 2026-06-01
-- Description: Adds strip_digits and prefix_to_add columns to routing_outbound_routes
--              for transforming destination numbers before sending to the gateway
--              (e.g., Lima fixed 3119898 -> 13119898 when the downstream softswitch
--              expects 8-digit numbers). Mirrors 007 for inbound routes. Applied at
--              the Kamailio gateway level (dr_gateways.strip / pri_prefix) via a
--              dedicated per-route gateway, so it takes effect AFTER drouting.

ALTER TABLE routing_outbound_routes
ADD COLUMN IF NOT EXISTS strip_digits INTEGER DEFAULT 0,
ADD COLUMN IF NOT EXISTS prefix_to_add VARCHAR(20) DEFAULT '';

COMMENT ON COLUMN routing_outbound_routes.strip_digits IS 'Number of leading digits to strip from the dialed number before sending to the gateway';
COMMENT ON COLUMN routing_outbound_routes.prefix_to_add IS 'Prefix to prepend to the dialed number after stripping (applied via the route gateway pri_prefix)';
