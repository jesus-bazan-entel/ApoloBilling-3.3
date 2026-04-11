-- Migration: Add number transformation fields to inbound routes
-- Date: 2026-04-09
-- Description: Adds strip_digits and prefix_to_add columns to routing_inbound_routes
--              for transforming destination numbers before bridge (e.g., 6722123456 -> 123456)

ALTER TABLE routing_inbound_routes
ADD COLUMN IF NOT EXISTS strip_digits INTEGER DEFAULT 0,
ADD COLUMN IF NOT EXISTS prefix_to_add VARCHAR(20) DEFAULT '';

-- Add comments for documentation
COMMENT ON COLUMN routing_inbound_routes.strip_digits IS 'Number of digits to strip from the beginning of destination_number before bridge';
COMMENT ON COLUMN routing_inbound_routes.prefix_to_add IS 'Prefix to add to destination_number after stripping digits';
