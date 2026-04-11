-- Migration: Unified Routing System
-- Date: 2026-03-21
-- Description: Sistema unificado de gestión de rutas para FreeSWITCH y Kamailio

-- ============================================
-- TABLA: routing_trunks
-- Representa carriers/gateways (antes dr_gateways en Kamailio)
-- ============================================

CREATE TABLE IF NOT EXISTS routing_trunks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,
    host VARCHAR(255) NOT NULL,
    port INTEGER DEFAULT 5060,
    transport VARCHAR(10) DEFAULT 'udp' CHECK (transport IN ('udp', 'tcp', 'tls')),
    auth_username VARCHAR(100),
    auth_password_encrypted BYTEA,
    strip_digits INTEGER DEFAULT 0,
    prefix_to_add VARCHAR(20) DEFAULT '',
    enabled BOOLEAN DEFAULT TRUE,
    -- Referencia a Kamailio dr_gateways (para sincronización)
    kamailio_gwid INTEGER,
    -- Estado de sincronización: pending, synced, error
    sync_status VARCHAR(20) DEFAULT 'pending',
    sync_error TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_routing_trunks_name ON routing_trunks(name);
CREATE INDEX idx_routing_trunks_enabled ON routing_trunks(enabled);
CREATE INDEX idx_routing_trunks_sync_status ON routing_trunks(sync_status);
CREATE INDEX idx_routing_trunks_kamailio_gwid ON routing_trunks(kamailio_gwid);

-- ============================================
-- TABLA: routing_trunk_groups
-- Grupos de failover (antes dr_gw_lists en Kamailio)
-- ============================================

CREATE TABLE IF NOT EXISTS routing_trunk_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,
    -- Estrategias: sequential, round_robin, weighted, least_calls
    failover_strategy VARCHAR(20) DEFAULT 'sequential' CHECK (failover_strategy IN ('sequential', 'round_robin', 'weighted', 'least_calls')),
    -- Referencia a Kamailio dr_gw_lists
    kamailio_group_id INTEGER,
    sync_status VARCHAR(20) DEFAULT 'pending',
    sync_error TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_routing_trunk_groups_name ON routing_trunk_groups(name);
CREATE INDEX idx_routing_trunk_groups_sync_status ON routing_trunk_groups(sync_status);

-- ============================================
-- TABLA: routing_trunk_group_members
-- Miembros del grupo con peso y prioridad
-- ============================================

CREATE TABLE IF NOT EXISTS routing_trunk_group_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES routing_trunk_groups(id) ON DELETE CASCADE,
    trunk_id UUID NOT NULL REFERENCES routing_trunks(id) ON DELETE CASCADE,
    priority INTEGER DEFAULT 1,          -- Para estrategia sequential (menor = primero)
    weight INTEGER DEFAULT 100,          -- Para estrategia weighted (porcentaje)
    max_channels INTEGER,                -- Límite de canales simultáneos
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(group_id, trunk_id)
);

CREATE INDEX idx_routing_trunk_group_members_group ON routing_trunk_group_members(group_id);
CREATE INDEX idx_routing_trunk_group_members_trunk ON routing_trunk_group_members(trunk_id);
CREATE INDEX idx_routing_trunk_group_members_priority ON routing_trunk_group_members(priority);

-- ============================================
-- TABLA: routing_outbound_routes
-- Rutas salientes (sincroniza con dr_rules de Kamailio)
-- ============================================

CREATE TABLE IF NOT EXISTS routing_outbound_routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,
    prefix_pattern VARCHAR(50) NOT NULL,
    priority INTEGER DEFAULT 100,
    -- Puede usar un grupo de trunks O un trunk individual (no ambos)
    trunk_group_id UUID REFERENCES routing_trunk_groups(id) ON DELETE SET NULL,
    trunk_id UUID REFERENCES routing_trunks(id) ON DELETE SET NULL,
    -- Reglas por horario (formato Kamailio timerec)
    -- Ejemplo: "* * * * 1-5 09:00-18:00" (L-V 9am-6pm)
    time_schedule VARCHAR(255),
    time_schedule_enabled BOOLEAN DEFAULT FALSE,
    enabled BOOLEAN DEFAULT TRUE,
    -- Referencia a Kamailio dr_rules
    kamailio_ruleid INTEGER,
    sync_status VARCHAR(20) DEFAULT 'pending',
    sync_error TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    -- Constraint: debe tener trunk_group_id O trunk_id (no ambos ni ninguno)
    CONSTRAINT routing_outbound_routes_destination_check CHECK (
        (trunk_group_id IS NOT NULL AND trunk_id IS NULL) OR
        (trunk_group_id IS NULL AND trunk_id IS NOT NULL)
    )
);

CREATE INDEX idx_routing_outbound_routes_name ON routing_outbound_routes(name);
CREATE INDEX idx_routing_outbound_routes_prefix ON routing_outbound_routes(prefix_pattern);
CREATE INDEX idx_routing_outbound_routes_priority ON routing_outbound_routes(priority);
CREATE INDEX idx_routing_outbound_routes_enabled ON routing_outbound_routes(enabled);
CREATE INDEX idx_routing_outbound_routes_sync_status ON routing_outbound_routes(sync_status);

-- ============================================
-- TABLA: routing_inbound_routes
-- Rutas entrantes (sincroniza con FreeSWITCH to-kamailio.xml)
-- ============================================

CREATE TABLE IF NOT EXISTS routing_inbound_routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,
    did_pattern VARCHAR(100) NOT NULL,      -- Patrón regex para destination_number
    source_ip_pattern VARCHAR(100),          -- Filtro por IP origen (network_addr)
    priority INTEGER DEFAULT 100,
    destination_host VARCHAR(255) NOT NULL,  -- IP/host destino del bridge
    destination_port INTEGER DEFAULT 5060,
    destination_profile VARCHAR(20) DEFAULT 'internal' CHECK (destination_profile IN ('internal', 'external')),
    call_timeout INTEGER DEFAULT 120,
    inherit_codec BOOLEAN DEFAULT TRUE,
    ignore_early_media BOOLEAN DEFAULT FALSE,
    bypass_media BOOLEAN DEFAULT FALSE,
    -- Destinos de failover (JSON array de {host, port})
    failover_destinations JSONB DEFAULT '[]',
    enabled BOOLEAN DEFAULT TRUE,
    -- Referencia al ID de extensión en FreeSWITCH XML
    freeswitch_extension_id VARCHAR(100),
    sync_status VARCHAR(20) DEFAULT 'pending',
    sync_error TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_routing_inbound_routes_name ON routing_inbound_routes(name);
CREATE INDEX idx_routing_inbound_routes_did ON routing_inbound_routes(did_pattern);
CREATE INDEX idx_routing_inbound_routes_priority ON routing_inbound_routes(priority);
CREATE INDEX idx_routing_inbound_routes_enabled ON routing_inbound_routes(enabled);
CREATE INDEX idx_routing_inbound_routes_sync_status ON routing_inbound_routes(sync_status);

-- ============================================
-- TABLA: routing_sync_log
-- Log de operaciones de sincronización
-- ============================================

CREATE TABLE IF NOT EXISTS routing_sync_log (
    id BIGSERIAL PRIMARY KEY,
    operation VARCHAR(50) NOT NULL,          -- create, update, delete, reload, migrate
    entity_type VARCHAR(50) NOT NULL,        -- trunk, trunk_group, outbound_route, inbound_route
    entity_id UUID,
    target_system VARCHAR(20) NOT NULL,      -- kamailio, freeswitch
    status VARCHAR(20) NOT NULL,             -- success, error, pending
    error_message TEXT,
    details JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_routing_sync_log_entity ON routing_sync_log(entity_type, entity_id);
CREATE INDEX idx_routing_sync_log_status ON routing_sync_log(status);
CREATE INDEX idx_routing_sync_log_created ON routing_sync_log(created_at);

-- ============================================
-- FUNCIÓN: Actualizar updated_at automáticamente
-- ============================================

CREATE OR REPLACE FUNCTION update_routing_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Triggers para actualizar updated_at
CREATE TRIGGER tr_routing_trunks_updated
    BEFORE UPDATE ON routing_trunks
    FOR EACH ROW EXECUTE FUNCTION update_routing_updated_at();

CREATE TRIGGER tr_routing_trunk_groups_updated
    BEFORE UPDATE ON routing_trunk_groups
    FOR EACH ROW EXECUTE FUNCTION update_routing_updated_at();

CREATE TRIGGER tr_routing_outbound_routes_updated
    BEFORE UPDATE ON routing_outbound_routes
    FOR EACH ROW EXECUTE FUNCTION update_routing_updated_at();

CREATE TRIGGER tr_routing_inbound_routes_updated
    BEFORE UPDATE ON routing_inbound_routes
    FOR EACH ROW EXECUTE FUNCTION update_routing_updated_at();

-- ============================================
-- COMENTARIOS
-- ============================================

COMMENT ON TABLE routing_trunks IS 'Trunks/carriers unificados (sincroniza con Kamailio dr_gateways)';
COMMENT ON TABLE routing_trunk_groups IS 'Grupos de failover (sincroniza con Kamailio dr_gw_lists)';
COMMENT ON TABLE routing_trunk_group_members IS 'Relación trunk-grupo con prioridad y peso';
COMMENT ON TABLE routing_outbound_routes IS 'Rutas salientes (sincroniza con Kamailio dr_rules)';
COMMENT ON TABLE routing_inbound_routes IS 'Rutas entrantes (sincroniza con FreeSWITCH to-kamailio.xml)';
COMMENT ON TABLE routing_sync_log IS 'Log de sincronización con sistemas externos';

COMMENT ON COLUMN routing_trunks.kamailio_gwid IS 'ID del gateway en Kamailio (dr_gateways.gwid)';
COMMENT ON COLUMN routing_trunk_groups.failover_strategy IS 'sequential=en orden, round_robin=rotativo, weighted=por peso, least_calls=menos ocupado';
COMMENT ON COLUMN routing_outbound_routes.time_schedule IS 'Formato timerec de Kamailio (ej: * * * * 1-5 09:00-18:00)';
COMMENT ON COLUMN routing_inbound_routes.failover_destinations IS 'Array JSON de destinos secundarios [{host, port}]';
