-- Migration: Direct inbound routing (Kamailio -> destination trunk, bypassing FreeSWITCH)
-- Date: 2026-10-24
-- Description: Until now every inbound DID went Carrier -> Kamailio -> FreeSWITCH-SBC
--              -> FreeSWITCH Legado, with FS-SBC parallel-forking to the Fonowin app
--              contact. Only a handful of numbers actually have the app, so for
--              99.9% of calls FS-SBC was a redundant B2BUA hop (double signalling,
--              double media anchoring).
--
--              With direct_routing = true the route's Kamailio dr_rule points at a
--              type=9 gateway on destination_trunk_id carrying the route's own
--              strip_digits / prefix_to_add, so Kamailio delivers the call straight
--              to the trunk. Numbers listed in fs_numbers (the post-transform number,
--              e.g. 16800110) are diverted by kamailio.cfg back through FreeSWITCH
--              (dr group 9100 -> kamailio_fs_ruleid) so the parallel ring to the app
--              keeps working.
--
--              Calls that skip FreeSWITCH produce no ESL events, so their CDRs come
--              from Kamailio's dialog-based acc CDRs (kamailio.acc_cdrs), imported by
--              rust-billing-engine; kamailio_cdr_sync_state holds that import cursor.

ALTER TABLE routing_inbound_routes
    ADD COLUMN IF NOT EXISTS direct_routing BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS fs_numbers TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS kamailio_fs_ruleid INTEGER;

COMMENT ON COLUMN routing_inbound_routes.direct_routing IS
    'Kamailio routes the DID straight to destination_trunk_id (strip/prefix applied by the gateway), skipping FreeSWITCH';
COMMENT ON COLUMN routing_inbound_routes.fs_numbers IS
    'Post-transform numbers that must still go through FreeSWITCH (e.g. Fonowin app users for parallel ring)';
COMMENT ON COLUMN routing_inbound_routes.kamailio_fs_ruleid IS
    'dr_rules.ruleid in group 9100 used to send fs_numbers to FreeSWITCH';

CREATE TABLE IF NOT EXISTS kamailio_cdr_sync_state (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    last_acc_cdr_id BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO kamailio_cdr_sync_state (id, last_acc_cdr_id) VALUES (1, 0)
ON CONFLICT (id) DO NOTHING;
