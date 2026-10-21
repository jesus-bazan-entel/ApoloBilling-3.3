-- 026: Rutas entrantes sin troncales legacy (routing_trunks)
--
-- 1) Los valores existentes quedan en blanco (el usuario las reasigna a un
--    dispositivo SIP tipo trunk desde la UI).
-- 2) Se elimina la columna y su FK hacia routing_trunks.
-- 3) Se elimina la fila legacy 'PBX Fibertel' de routing_trunks.
--    NO se toca 'Trunk Integratel Principal': la usan las rutas salientes,
--    el grupo Integratel_Principal y Kamailio (gwid 60).

UPDATE inbound_routes SET gateway_id = NULL WHERE gateway_id IS NOT NULL;

ALTER TABLE inbound_routes DROP CONSTRAINT IF EXISTS inbound_routes_gateway_id_fkey;
ALTER TABLE inbound_routes DROP CONSTRAINT IF EXISTS chk_inbound_route_single_dest;
ALTER TABLE inbound_routes DROP COLUMN IF EXISTS gateway_id;

DELETE FROM routing_trunks WHERE name = 'PBX Fibertel';
