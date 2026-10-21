-- Migration 036: Real client source IP on active_calls
-- Mismo motivo que cdrs.source_ip (migration 034): para trafico relay-eado
-- por Kamailio (troncales IP, FLB_SRC_LANEXT), el billing engine ve la IP
-- del SBC, no la de la troncal real, salvo que use el header X-Orig-Src-IP.
-- Esta columna permite contar canales activos POR TRONCAL IP (sip_endpoints
-- no tiene sip_username para ellas, asi que no se puede contar por numero
-- como con las extensiones) - ver check_endpoint_concurrent_limit.

BEGIN;

ALTER TABLE active_calls ADD COLUMN IF NOT EXISTS source_ip VARCHAR(45);

COMMIT;
