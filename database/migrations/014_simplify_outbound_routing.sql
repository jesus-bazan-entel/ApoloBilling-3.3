-- ============================================
-- Migration 014: Simplify Outbound Routing
-- Model based on dSIPRouter for carriers/endpoints
-- ============================================

BEGIN;

-- ============================================
-- TABLA: carriers (dr_gateways type=8)
-- Proveedores SIP externos
-- ============================================
CREATE TABLE IF NOT EXISTS carriers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,

    -- Conexión SIP
    host VARCHAR(255) NOT NULL,
    port INTEGER DEFAULT 5060,
    transport VARCHAR(10) DEFAULT 'udp' CHECK (transport IN ('udp', 'tcp', 'tls')),

    -- Autenticación (opcional)
    auth_username VARCHAR(100),
    auth_password VARCHAR(255),

    -- Transformación de número
    strip_digits INTEGER DEFAULT 0,
    prefix_to_add VARCHAR(20) DEFAULT '',

    -- Kamailio sync
    kamailio_gwid INTEGER,
    sync_status VARCHAR(20) DEFAULT 'pending' CHECK (sync_status IN ('pending', 'synced', 'error')),
    sync_error TEXT,

    -- Monitoreo SIP OPTIONS
    sip_status VARCHAR(20) DEFAULT 'unknown' CHECK (sip_status IN ('unknown', 'reachable', 'unreachable', 'checking')),
    last_options_check TIMESTAMPTZ,
    last_options_latency_ms INTEGER,

    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- ============================================
-- TABLA: endpoints (dr_gateways type=9)
-- Conexiones PBX internas
-- ============================================
CREATE TABLE IF NOT EXISTS endpoints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,

    host VARCHAR(255) NOT NULL,
    port INTEGER DEFAULT 5060,
    transport VARCHAR(10) DEFAULT 'udp' CHECK (transport IN ('udp', 'tcp', 'tls')),

    -- Config FreeSWITCH
    fs_profile VARCHAR(50) DEFAULT 'internal',

    -- Kamailio sync
    kamailio_gwid INTEGER,
    sync_status VARCHAR(20) DEFAULT 'pending' CHECK (sync_status IN ('pending', 'synced', 'error')),
    sync_error TEXT,

    enabled BOOLEAN DEFAULT true,
    is_default BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Solo un endpoint puede ser default
CREATE UNIQUE INDEX IF NOT EXISTS idx_endpoints_single_default
ON endpoints (is_default) WHERE is_default = true;

-- ============================================
-- TABLA: carrier_groups (dr_gw_lists)
-- Grupos para failover
-- ============================================
CREATE TABLE IF NOT EXISTS carrier_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,

    strategy VARCHAR(20) DEFAULT 'sequential'
        CHECK (strategy IN ('sequential', 'round_robin', 'weighted')),

    kamailio_group_id INTEGER,
    sync_status VARCHAR(20) DEFAULT 'pending' CHECK (sync_status IN ('pending', 'synced', 'error')),
    sync_error TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- ============================================
-- TABLA: carrier_group_members
-- ============================================
CREATE TABLE IF NOT EXISTS carrier_group_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES carrier_groups(id) ON DELETE CASCADE,
    carrier_id UUID NOT NULL REFERENCES carriers(id) ON DELETE CASCADE,
    priority INTEGER DEFAULT 1,
    weight INTEGER DEFAULT 100,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(group_id, carrier_id)
);

-- ============================================
-- TABLA: outbound_routes (dr_rules)
-- Auto-prioridad por longitud de prefijo
-- ============================================
CREATE TABLE IF NOT EXISTS outbound_routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,

    prefix VARCHAR(30) NOT NULL UNIQUE,

    -- Destino: grupo O carrier individual
    carrier_group_id UUID REFERENCES carrier_groups(id) ON DELETE SET NULL,
    carrier_id UUID REFERENCES carriers(id) ON DELETE SET NULL,

    kamailio_ruleid INTEGER,
    sync_status VARCHAR(20) DEFAULT 'pending' CHECK (sync_status IN ('pending', 'synced', 'error')),
    sync_error TEXT,

    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT outbound_routes_dest_check CHECK (
        (carrier_group_id IS NOT NULL AND carrier_id IS NULL) OR
        (carrier_group_id IS NULL AND carrier_id IS NOT NULL)
    )
);

-- Auto-prioridad: prefijos más largos primero
CREATE INDEX IF NOT EXISTS idx_outbound_routes_prefix_len ON outbound_routes(LENGTH(prefix) DESC, prefix);
CREATE INDEX IF NOT EXISTS idx_outbound_routes_enabled ON outbound_routes(enabled) WHERE enabled = true;

-- Índices para carriers
CREATE INDEX IF NOT EXISTS idx_carriers_enabled ON carriers(enabled) WHERE enabled = true;
CREATE INDEX IF NOT EXISTS idx_carriers_sip_status ON carriers(sip_status);

-- Índices para endpoints
CREATE INDEX IF NOT EXISTS idx_endpoints_enabled ON endpoints(enabled) WHERE enabled = true;

-- Índices para carrier_group_members
CREATE INDEX IF NOT EXISTS idx_carrier_group_members_group ON carrier_group_members(group_id);
CREATE INDEX IF NOT EXISTS idx_carrier_group_members_carrier ON carrier_group_members(carrier_id);

-- ============================================
-- MIGRACIÓN DE DATOS desde tablas existentes
-- ============================================

-- Migrar routing_trunks (public) → carriers
-- Note: auth_password_encrypted is bytea, we store a placeholder since it can't be decrypted
INSERT INTO carriers (
    name, description, host, port, transport,
    auth_username, auth_password, strip_digits, prefix_to_add,
    kamailio_gwid, sync_status, sip_status, enabled, created_at, updated_at
)
SELECT
    name, description, host, port, transport,
    auth_username, '', -- Password must be re-entered (encrypted in source)
    COALESCE(strip_digits, 0), COALESCE(prefix_to_add, ''),
    kamailio_gwid,
    CASE WHEN sync_status IS NOT NULL THEN sync_status ELSE 'pending' END,
    COALESCE(sip_status, 'unknown'),
    enabled, created_at, updated_at
FROM routing_trunks
WHERE trunk_type = 'public'
ON CONFLICT (name) DO NOTHING;

-- Migrar routing_trunks (private) → endpoints
-- Note: routing_trunks doesn't have sip_profile, using freeswitch_gateway_name or default
INSERT INTO endpoints (
    name, description, host, port, transport,
    fs_profile, kamailio_gwid, sync_status, enabled, created_at, updated_at
)
SELECT
    name, description, host, port, transport,
    'internal', -- Default profile for endpoints
    kamailio_gwid,
    CASE WHEN sync_status IS NOT NULL THEN sync_status ELSE 'pending' END,
    enabled, created_at, updated_at
FROM routing_trunks
WHERE trunk_type = 'private'
ON CONFLICT (name) DO NOTHING;

-- Migrar routing_trunk_groups → carrier_groups
INSERT INTO carrier_groups (
    name, description, strategy, kamailio_group_id, sync_status, created_at, updated_at
)
SELECT
    name, description,
    CASE
        WHEN failover_strategy = 'sequential' THEN 'sequential'
        WHEN failover_strategy = 'round_robin' THEN 'round_robin'
        WHEN failover_strategy = 'weighted' THEN 'weighted'
        ELSE 'sequential'
    END,
    kamailio_group_id,
    CASE WHEN sync_status IS NOT NULL THEN sync_status ELSE 'pending' END,
    created_at, updated_at
FROM routing_trunk_groups
ON CONFLICT (name) DO NOTHING;

-- Migrar routing_trunk_group_members → carrier_group_members
-- Necesitamos mapear old trunk_id a new carrier_id
INSERT INTO carrier_group_members (group_id, carrier_id, priority, weight)
SELECT
    cg.id,
    c.id,
    rtgm.priority,
    rtgm.weight
FROM routing_trunk_group_members rtgm
JOIN routing_trunks rt ON rtgm.trunk_id = rt.id
JOIN carriers c ON c.name = rt.name
JOIN routing_trunk_groups rtg ON rtgm.group_id = rtg.id
JOIN carrier_groups cg ON cg.name = rtg.name
WHERE rt.trunk_type = 'public'
ON CONFLICT (group_id, carrier_id) DO NOTHING;

-- Migrar routing_outbound_routes → outbound_routes
-- Con referencia a carrier_group_id
INSERT INTO outbound_routes (
    name, description, prefix, carrier_group_id,
    kamailio_ruleid, sync_status, enabled, created_at, updated_at
)
SELECT
    ror.name, ror.description, ror.prefix_pattern,
    cg.id,
    ror.kamailio_ruleid,
    CASE WHEN ror.sync_status IS NOT NULL THEN ror.sync_status ELSE 'pending' END,
    ror.enabled, ror.created_at, ror.updated_at
FROM routing_outbound_routes ror
JOIN routing_trunk_groups rtg ON ror.trunk_group_id = rtg.id
JOIN carrier_groups cg ON cg.name = rtg.name
ON CONFLICT (prefix) DO NOTHING;

-- ============================================
-- Funciones de actualización de timestamps
-- ============================================

CREATE OR REPLACE FUNCTION update_carriers_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION update_endpoints_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION update_carrier_groups_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION update_outbound_routes_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Triggers para updated_at
DROP TRIGGER IF EXISTS carriers_updated_at ON carriers;
CREATE TRIGGER carriers_updated_at
    BEFORE UPDATE ON carriers
    FOR EACH ROW
    EXECUTE FUNCTION update_carriers_updated_at();

DROP TRIGGER IF EXISTS endpoints_updated_at ON endpoints;
CREATE TRIGGER endpoints_updated_at
    BEFORE UPDATE ON endpoints
    FOR EACH ROW
    EXECUTE FUNCTION update_endpoints_updated_at();

DROP TRIGGER IF EXISTS carrier_groups_updated_at ON carrier_groups;
CREATE TRIGGER carrier_groups_updated_at
    BEFORE UPDATE ON carrier_groups
    FOR EACH ROW
    EXECUTE FUNCTION update_carrier_groups_updated_at();

DROP TRIGGER IF EXISTS outbound_routes_updated_at ON outbound_routes;
CREATE TRIGGER outbound_routes_updated_at
    BEFORE UPDATE ON outbound_routes
    FOR EACH ROW
    EXECUTE FUNCTION update_outbound_routes_updated_at();

COMMIT;
