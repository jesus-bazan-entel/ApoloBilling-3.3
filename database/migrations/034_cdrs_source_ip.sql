-- Migration 034: Real client source IP on CDRs
-- Para llamadas relay-eadas por Kamailio (FLB_SRC_LANEXT -> RELAY_LANEXT,
-- troncales IP), el peer de red que FreeSWITCH ve es Kamailio mismo (el
-- SBC), no la PBX real - Network-IP/variable_sip_*_ip quedan con la IP del
-- SBC. Kamailio ahora inyecta el header X-Orig-Src-IP con la IP real antes
-- de relayar (ver kamailio.cfg REQINIT, clasificacion FLT_PBX), y
-- event.source_ip() lo prioriza. Esta columna persiste ese valor en el CDR.

BEGIN;

ALTER TABLE cdrs ADD COLUMN IF NOT EXISTS source_ip VARCHAR(45);

-- (sin COMMENT ON COLUMN: apolo_user no es owner de cdrs en esta instalacion)

COMMIT;
