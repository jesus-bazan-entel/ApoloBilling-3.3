-- Migration: Simplify Inbound Routing
-- Date: 2026-08-16
-- Description: Replace complex Carrier → DID Override model with simple inbound_routes table
--              Auto-priority is determined by pattern length (longer patterns = higher priority)

-- ============================================
-- TABLA: inbound_routes
-- Rutas entrantes simplificadas con auto-prioridad
-- ============================================

CREATE TABLE IF NOT EXISTS inbound_routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,

    -- Patron de matching (DID exacto o prefijo con *)
    -- Ejemplos: "672017402000" (exacto), "6720*" (wildcard prefijo)
    pattern VARCHAR(30) NOT NULL UNIQUE,
    source_ip VARCHAR(255),               -- Filtro IP opcional (carrier source)

    -- Destino
    destination_type VARCHAR(20) NOT NULL DEFAULT 'extension'
        CHECK (destination_type IN ('extension', 'gateway')),
    gateway_id UUID REFERENCES routing_trunks(id) ON DELETE SET NULL,
    destination_host VARCHAR(255) DEFAULT '10.10.22.4',

    -- Transformacion de numero
    strip_digits INT DEFAULT 4,           -- Digitos a quitar del inicio
    prefix_to_add VARCHAR(20) DEFAULT '', -- Prefijo a agregar despues de strip

    -- Opciones de senalizacion
    call_timeout INT DEFAULT 55,          -- Timeout en segundos
    ims_mode BOOLEAN DEFAULT false,       -- 100rel/PRACK + timeout handlers
    early_media BOOLEAN DEFAULT false,    -- 183 Session Progress con ringback

    enabled BOOLEAN DEFAULT true,
    sync_status VARCHAR(20) DEFAULT 'pending',
    sync_error TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Indice principal: ordenar por longitud de patron DESC para auto-prioridad
-- Patrones mas largos (mas especificos) se procesan primero
CREATE INDEX idx_inbound_routes_pattern_len ON inbound_routes(LENGTH(pattern) DESC, pattern);
CREATE INDEX idx_inbound_routes_enabled ON inbound_routes(enabled);
CREATE INDEX idx_inbound_routes_sync_status ON inbound_routes(sync_status);

-- ============================================
-- TRIGGER: Actualizar updated_at
-- ============================================

CREATE OR REPLACE FUNCTION update_inbound_routes_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER tr_inbound_routes_updated
    BEFORE UPDATE ON inbound_routes
    FOR EACH ROW EXECUTE FUNCTION update_inbound_routes_updated_at();

-- ============================================
-- MIGRACION DE DATOS: DID Overrides -> inbound_routes
-- Los DID Overrides son las rutas realmente usadas
-- ============================================

INSERT INTO inbound_routes (
    name, description, pattern, source_ip,
    destination_type, gateway_id, destination_host,
    strip_digits, prefix_to_add,
    call_timeout, ims_mode, early_media,
    enabled, sync_status
)
SELECT
    COALESCE(d.name, 'DID ' || d.did),
    d.description,
    d.did,  -- Patron exacto (sin wildcard)
    NULL,   -- source_ip no existia en DID overrides
    d.destination_type,
    d.gateway_id,
    COALESCE(d.destination_host, c.default_destination_host, '10.10.22.4'),
    COALESCE(d.strip_digits, c.strip_digits, 4),
    COALESCE(d.prefix_to_add, c.prefix_to_add, ''),
    COALESCE(d.call_timeout, c.call_timeout, 55),
    COALESCE(d.ims_mode, c.ims_mode, false),
    COALESCE(d.send_early_media, c.send_early_media, false),
    d.enabled,
    'pending'
FROM routing_did_overrides d
JOIN routing_carriers c ON d.carrier_id = c.id
ON CONFLICT (pattern) DO NOTHING;

-- ============================================
-- MIGRACION DE DATOS: Carriers -> inbound_routes (wildcards)
-- Carriers se convierten en patrones con wildcard
-- ============================================

INSERT INTO inbound_routes (
    name, description, pattern, source_ip,
    destination_type, gateway_id, destination_host,
    strip_digits, prefix_to_add,
    call_timeout, ims_mode, early_media,
    enabled, sync_status
)
SELECT
    c.name,
    c.description,
    c.prefix || '*',  -- Patron wildcard (ej: 6720*)
    c.source_ip,
    c.default_destination_type,
    c.default_gateway_id,
    COALESCE(c.default_destination_host, '10.10.22.4'),
    c.strip_digits,
    COALESCE(c.prefix_to_add, ''),
    COALESCE(c.call_timeout, 55),
    c.ims_mode,
    c.send_early_media,
    c.enabled,
    'pending'
FROM routing_carriers c
WHERE NOT EXISTS (
    SELECT 1 FROM inbound_routes ir WHERE ir.pattern = c.prefix || '*'
)
ON CONFLICT (pattern) DO NOTHING;

-- ============================================
-- MARCAR TABLAS VIEJAS COMO DEPRECATED
-- ============================================

COMMENT ON TABLE routing_carriers IS
'DEPRECATED: Use inbound_routes instead.
Migrated to inbound_routes table with pattern wildcards (prefix*).
Kept for backward compatibility and rollback.';

COMMENT ON TABLE routing_did_overrides IS
'DEPRECATED: Use inbound_routes instead.
Migrated to inbound_routes table with exact patterns.
Kept for backward compatibility and rollback.';

-- ============================================
-- DOCUMENTACION
-- ============================================

COMMENT ON TABLE inbound_routes IS
'Rutas de llamadas entrantes simplificadas.
Auto-prioridad: patrones mas largos ganan (672017402000 > 6720*).
Patrones: numero exacto "672017402000" o wildcard "6720*".';

COMMENT ON COLUMN inbound_routes.pattern IS
'Patron de matching para el DID.
- Exacto: "672017402000" (match solo ese numero)
- Wildcard: "6720*" (match 6720 seguido de cualquier digito)
Auto-prioridad por longitud: patrones mas largos se procesan primero.';

COMMENT ON COLUMN inbound_routes.destination_type IS
'extension: enruta a extensiones locales (user/{number}@host)
gateway: enruta via gateway configurado (sofia/gateway/{name}/{number})';

COMMENT ON COLUMN inbound_routes.ims_mode IS
'Modo IMS/3GPP: habilita 100rel/PRACK y handlers de timeout para carriers IMS.
Requerido para carriers como INTEGRATEL que esperan respuestas fiables.';

COMMENT ON COLUMN inbound_routes.early_media IS
'Enviar early media (183 Session Progress con ringback).
Util para reproducir tono de espera mientras se conecta la llamada.';
