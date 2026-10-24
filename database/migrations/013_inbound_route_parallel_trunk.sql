-- Migration: Parallel-ring destination as a trunk reference for inbound routes
-- Date: 2026-06-01
-- Description: Replaces the ring_registered_parallel boolean (012) with a trunk
--              reference. When set, an inbound call rings the primary destination
--              AND this trunk's gateway simultaneously (first to answer wins).
--              Used to fork to the SBC Push trunk (private IP 10.118.0.5) which
--              orchestrates the FCM push and mobile delivery. The 012 boolean
--              column is left in place (orphaned) to avoid a destructive drop;
--              the application no longer reads it.

ALTER TABLE routing_inbound_routes
ADD COLUMN IF NOT EXISTS parallel_trunk_id UUID REFERENCES routing_trunks(id) ON DELETE SET NULL;

COMMENT ON COLUMN routing_inbound_routes.parallel_trunk_id IS 'Optional trunk to ring in parallel with the primary destination (e.g. SBC Push). First to answer wins.';
