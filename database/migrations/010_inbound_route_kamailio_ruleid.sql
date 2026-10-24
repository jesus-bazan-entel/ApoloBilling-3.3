-- Migration: Add kamailio_ruleid to routing_inbound_routes
-- Date: 2026-04-18
-- Description: Tracks the dr_rules.ruleid in Kamailio's MySQL that corresponds
--              to each inbound route, so the sync process can update or
--              delete the matching rule when the Postgres row changes.
--              Mirrors the same column already present on routing_outbound_routes.

ALTER TABLE routing_inbound_routes
    ADD COLUMN IF NOT EXISTS kamailio_ruleid INTEGER;

COMMENT ON COLUMN routing_inbound_routes.kamailio_ruleid IS
    'ruleid in Kamailio dr_rules (groupid=9000) that corresponds to this inbound route. NULL when the route has never been synced to Kamailio.';
