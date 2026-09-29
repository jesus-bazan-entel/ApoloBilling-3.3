-- Migration: Carrier-based Routing System
-- Date: 2026-08-15
-- Description: Simplified carrier-class routing with auto-priority
--              Replaces manual priority management with Carrier → DID hierarchy

-- ============================================
-- TABLA: routing_carriers
-- Carriers con prefijo y configuración por defecto
-- ============================================

CREATE TABLE IF NOT EXISTS routing_carriers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,

    -- Identificación del carrier
    prefix VARCHAR(20) NOT NULL UNIQUE,       -- "6720", "6722" - genera regex ^(prefix.+)$
    source_ip VARCHAR(255),                   -- IP del carrier (opcional, para filtrar)

    -- Destino por defecto para llamadas de este carrier
    default_destination_type VARCHAR(20) NOT NULL DEFAULT 'extension'
        CHECK (default_destination_type IN ('extension', 'gateway')),
    -- 'extension' = user/{number}@host (extensiones locales)
    -- 'gateway' = sofia/gateway/{gateway_name}/{number}

    default_gateway_id UUID REFERENCES routing_trunks(id) ON DELETE SET NULL,
    default_destination_host VARCHAR(255) DEFAULT '10.10.22.4',
    default_destination_profile VARCHAR(50) DEFAULT 'internal',

    -- Transformación de número por defecto
    strip_digits INT DEFAULT 4,               -- Quitar prefijo carrier (6720 = 4 dígitos)
    prefix_to_add VARCHAR(20) DEFAULT '',

    -- Opciones de señalización
    ims_mode BOOLEAN DEFAULT false,           -- 100rel/PRACK + timeout handlers
    send_early_media BOOLEAN DEFAULT false,   -- 183 Session Progress con ringback
    call_timeout INT DEFAULT 120,

    enabled BOOLEAN DEFAULT true,
    sync_status VARCHAR(20) DEFAULT 'pending',
    sync_error TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_routing_carriers_prefix ON routing_carriers(prefix);
CREATE INDEX idx_routing_carriers_enabled ON routing_carriers(enabled);
CREATE INDEX idx_routing_carriers_sync_status ON routing_carriers(sync_status);

-- ============================================
-- TABLA: routing_did_overrides
-- Excepciones específicas para DIDs individuales
-- ============================================

CREATE TABLE IF NOT EXISTS routing_did_overrides (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    carrier_id UUID NOT NULL REFERENCES routing_carriers(id) ON DELETE CASCADE,

    -- DID específico (número exacto, no regex)
    did VARCHAR(30) NOT NULL,                 -- "672017402000"
    name VARCHAR(100),                        -- Nombre descriptivo
    description TEXT,

    -- Destino (sobrescribe el default del carrier)
    destination_type VARCHAR(20) NOT NULL
        CHECK (destination_type IN ('extension', 'gateway')),
    gateway_id UUID REFERENCES routing_trunks(id) ON DELETE SET NULL,
    destination_extension VARCHAR(50),        -- Para tipo 'extension'
    destination_host VARCHAR(255),            -- Para destino directo
    destination_profile VARCHAR(50) DEFAULT 'internal',

    -- Transformación (sobrescribe el default del carrier)
    strip_digits INT,                         -- NULL = heredar de carrier
    prefix_to_add VARCHAR(20),

    -- Opciones (NULL = heredar de carrier)
    ims_mode BOOLEAN,
    send_early_media BOOLEAN,
    call_timeout INT,

    enabled BOOLEAN DEFAULT true,
    sync_status VARCHAR(20) DEFAULT 'pending',
    sync_error TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(carrier_id, did)
);

CREATE INDEX idx_routing_did_overrides_carrier ON routing_did_overrides(carrier_id);
CREATE INDEX idx_routing_did_overrides_did ON routing_did_overrides(did);
CREATE INDEX idx_routing_did_overrides_enabled ON routing_did_overrides(enabled);

-- ============================================
-- TRIGGERS: Actualizar updated_at
-- ============================================

CREATE TRIGGER tr_routing_carriers_updated
    BEFORE UPDATE ON routing_carriers
    FOR EACH ROW EXECUTE FUNCTION update_routing_updated_at();

CREATE TRIGGER tr_routing_did_overrides_updated
    BEFORE UPDATE ON routing_did_overrides
    FOR EACH ROW EXECUTE FUNCTION update_routing_updated_at();

-- ============================================
-- MIGRACIÓN DE DATOS EXISTENTES
-- ============================================

-- 1. Crear carriers a partir de rutas con patrones genéricos
INSERT INTO routing_carriers (
    name, description, prefix, source_ip,
    default_destination_type, default_gateway_id, default_destination_host, default_destination_profile,
    strip_digits, prefix_to_add,
    ims_mode, send_early_media, call_timeout,
    enabled, sync_status
)
SELECT DISTINCT ON (extracted_prefix)
    r.name,
    r.description,
    -- Extraer prefijo del patrón regex: ^(6720.+)$ -> 6720
    REGEXP_REPLACE(r.did_pattern, '^\^\(([0-9]+)\.?\+?\)\$$', '\1') as extracted_prefix,
    r.source_ip_pattern,
    CASE
        WHEN r.destination_trunk_id IS NOT NULL THEN 'gateway'
        ELSE 'extension'
    END,
    r.destination_trunk_id,
    r.destination_host,
    r.destination_profile,
    COALESCE(r.strip_digits, 4),
    COALESCE(r.prefix_to_add, ''),
    COALESCE(r.ims_mode, false),
    COALESCE(r.send_early_media, false),
    COALESCE(r.call_timeout, 120),
    r.enabled,
    'pending'
FROM routing_inbound_routes r
WHERE r.did_pattern ~ '^\^\([0-9]+\.\+\)\$$'  -- Solo patrones genéricos como ^(6720.+)$
ORDER BY extracted_prefix, r.priority;

-- 2. Crear DID overrides a partir de rutas con patrones específicos
INSERT INTO routing_did_overrides (
    carrier_id, did, name, description,
    destination_type, gateway_id, destination_host, destination_profile,
    strip_digits, prefix_to_add,
    ims_mode, send_early_media, call_timeout,
    enabled, sync_status
)
SELECT
    c.id,
    -- Extraer DID del patrón regex: ^(672017402000)$ -> 672017402000
    REGEXP_REPLACE(r.did_pattern, '^\^\(([0-9]+)\)\$$', '\1') as extracted_did,
    r.name,
    r.description,
    CASE
        WHEN r.destination_trunk_id IS NOT NULL THEN 'gateway'
        ELSE 'extension'
    END,
    r.destination_trunk_id,
    r.destination_host,
    r.destination_profile,
    r.strip_digits,
    r.prefix_to_add,
    r.ims_mode,
    r.send_early_media,
    r.call_timeout,
    r.enabled,
    'pending'
FROM routing_inbound_routes r
JOIN routing_carriers c ON
    REGEXP_REPLACE(r.did_pattern, '^\^\(([0-9]+)\)\$$', '\1') LIKE c.prefix || '%'
WHERE r.did_pattern ~ '^\^\([0-9]+\)\$$'  -- Solo patrones específicos como ^(672017402000)$
AND r.did_pattern !~ '^\^\([0-9]+\.\+\)\$$';  -- Excluir patrones genéricos

-- ============================================
-- MARCAR TABLA VIEJA COMO DEPRECATED
-- ============================================

COMMENT ON TABLE routing_inbound_routes IS
'DEPRECATED: Use routing_carriers and routing_did_overrides instead.
This table is kept for backward compatibility and rollback purposes.';

-- ============================================
-- COMENTARIOS DOCUMENTACIÓN
-- ============================================

COMMENT ON TABLE routing_carriers IS
'Carriers de telefonía con configuración por defecto.
El prefijo define qué llamadas pertenecen a este carrier.
Ejemplo: ENTEL con prefijo 6720 maneja todas las llamadas 6720*.';

COMMENT ON TABLE routing_did_overrides IS
'Excepciones de enrutamiento para DIDs específicos.
Sobrescriben la configuración del carrier padre.
Siempre se procesan ANTES que los carriers genéricos (auto-prioridad).';

COMMENT ON COLUMN routing_carriers.prefix IS
'Prefijo numérico del carrier. Genera automáticamente el regex ^(prefix.+)$';

COMMENT ON COLUMN routing_carriers.default_destination_type IS
'extension: enruta a extensiones locales (user/). gateway: enruta via gateway configurado.';

COMMENT ON COLUMN routing_did_overrides.did IS
'Número DID completo (ej: 672017402000). Se genera regex ^(did)$ automáticamente.';
