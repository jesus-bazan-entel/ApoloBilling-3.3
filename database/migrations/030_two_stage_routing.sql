-- Migration 030: Two-stage inbound routing (normalize first, decide after)
-- Date: 2026-10-03
-- Description: Modelo operador (estilo DNSET por etapas):
--   ETAPA 1 (contexto to-kamailio): exactas = overrides con destino fijo;
--     wildcards "RN*" = CLASIFICADORES (normalizan RN->nacional, aplican la
--     clase de carrier timeout/ims/early-media y transfieren, NO puentean);
--     passthrough = lo que ya viene en formato nacional.
--   ETAPA 2 (contexto national-routing): decide sobre el numero NACIONAL:
--     L2 registrada -> L3 rango de su PBX -> L4 480.
--   Resultado: cero-config por DID. Un DID nuevo no necesita fila; un carrier
--   (RN) nuevo necesita UNA wildcard; solo los servicios especiales llevan exacta.

BEGIN;

INSERT INTO system_settings (key, value, description) VALUES
('inbound.two_stage', 'true',
 'ON: wildcards RN* clasifican+transfieren a national-routing (cero-config por DID). OFF: comportamiento legacy (wildcards puentean directo).'),
('inbound.default_timeout', '55',
 'Timeout (s) del clasificador passthrough (trafico que ya viene en formato nacional).')
ON CONFLICT (key) DO NOTHING;

COMMIT;
