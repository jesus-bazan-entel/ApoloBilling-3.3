-- Migration: Add IMS mode support to inbound routes
-- Date: 2026-08-10
-- Description: Adds ims_mode column for IMS interworking (100rel/PRACK, timeout handlers, congestion tones)

-- Add ims_mode column to routing_inbound_routes
ALTER TABLE routing_inbound_routes
ADD COLUMN IF NOT EXISTS ims_mode BOOLEAN NOT NULL DEFAULT false;

COMMENT ON COLUMN routing_inbound_routes.ims_mode IS 'Enable IMS interworking mode: 100rel/PRACK, timeout handlers, congestion tones';

-- Update existing routes that should have IMS mode enabled (carriers like INTEGRATEL, ENTEL)
-- Routes with prefixes 6720 (ENTEL), 6722 (INTEGRATEL) typically need IMS mode
UPDATE routing_inbound_routes
SET ims_mode = true
WHERE did_pattern LIKE '%672%' OR did_pattern LIKE '%6720%' OR did_pattern LIKE '%6722%';
