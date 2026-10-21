-- Migration 027: Inbound Auto-Routing (carrier-class)
-- Date: 2026-10-03
-- Description: Enrutamiento inteligente entrante estilo carrier (Vodafone/AT&T).
--   Las Rutas Entrantes pasan a ser SOLO overrides/excepciones.
--   El dialplan generado (to-kamailio.xml) resuelve en este orden:
--     L1: Overrides explicitos (routing_inbound_routes, por priority/did_pattern)
--     L2: Extensiones locales registradas (sofia_contact -> user/...).
--         No requiere configurar cada DID: si el destino esta registrado, entra.
--     L3: PBXs conectadas (routing_trunks privados habilitados, caza automatica).
--         No requiere configurar cada DID: prueba cada gateway PBX en orden.
--     L4: Fallback configurable (480/404/503 via system_settings).
--
--   Con esto, el operador solo configura:
--     a) Cero-config: nada (L2+L3 cubren extensiones y PBXs), o
--     b) Una ruta por PREFIJO/PBX (ej: ^6722(.+)$ -> pbx-fibertel con strip),
--        en lugar de una ruta por cada DID/linea.
--   Kamailio ya es catch-all en inbound (group 9000 -> FS), no necesita por-DID.

BEGIN;

-- Asegurar tabla system_settings (por si migraciones viejas no la crearon)
CREATE TABLE IF NOT EXISTS system_settings (
    key VARCHAR(255) PRIMARY KEY,
    value TEXT NOT NULL,
    description TEXT,
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Interruptores del enrutamiento automatico
INSERT INTO system_settings (key, value, description) VALUES
('inbound.auto_local_extensions', 'true',
 'L2: Si true, el dialplan entrante prueba extensiones locales registradas (sofia_contact) antes de PBXs. Cero-config por DID.'),
('inbound.auto_pbx_hunt', 'true',
 'L3: Si true, el dialplan entrante hace hunt automatico sobre trunks privados habilitados (PBXs). Cero-config por DID.'),
('inbound.local_timeout', '30',
 'Timeout (s) para bridge a extension local registrada.'),
('inbound.pbx_hunt_timeout', '55',
 'Timeout (s) por pata en hunt a PBXs. Debe ser < timeout del carrier (ej: 62s INTEGRATEL).'),
('inbound.default_action', '480',
 'L4 fallback cuando nada matchea: 480 (Temporarily Unavailable), 404 (Not Found) o 503 (Service Unavailable). Carrier-class usa 480/503 con tono, no 403.'),
('inbound.pbx_strip_digits', '0',
 'Strip por defecto aplicado en hunt automatico a PBX (0 = enviar DID completo; cada trunk puede definir su propio strip_digits que tiene prioridad).'),
('inbound.pbx_prefix_to_add', '',
 'Prefijo por defecto agregado en hunt automatico a PBX (vacio = sin prefijo).')
ON CONFLICT (key) DO NOTHING;

-- Documentar el modelo carrier-class en la tabla de rutas
COMMENT ON TABLE routing_inbound_routes IS
'OVERRIDES/excepciones (L1). El enrutamiento normal es AUTOMATICO: L2 extensiones registradas -> L3 hunt PBXs (trunks privados) -> L4 fallback (system_settings inbound.*). Solo crear rutas aqui para casos especiales (strip/prefix/timeout/early-media/IMS distintos del default, o forzar un DID a un destino fijo).';

COMMIT;
