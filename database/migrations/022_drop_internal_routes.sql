-- Drop the "Rutas Internas" tables (internal_routes, system_endpoints).
-- Their sync was a stub that only flagged rows as synced: nothing ever reached
-- the FreeSWITCH dialplan or Kamailio dr_gateways. The real FS <-> Kamailio links
-- are private trunks (type=9 gateways), dr_rules and the generated from-kamailio.xml.
-- Menu, API and code removed on 2026-10-25.

DROP TABLE IF EXISTS internal_routes;
DROP TABLE IF EXISTS system_endpoints;
