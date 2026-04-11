-- Migration: 006_internal_routes.sql
-- Description: Internal routes between FreeSWITCH and Kamailio

-- Table for system endpoints (FreeSWITCH and Kamailio connection points)
CREATE TABLE IF NOT EXISTS system_endpoints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    endpoint_type VARCHAR(30) NOT NULL CHECK (endpoint_type IN ('freeswitch', 'kamailio')),
    description TEXT,

    -- Connection details
    ip_address VARCHAR(45) NOT NULL,
    port INTEGER NOT NULL,
    transport VARCHAR(10) DEFAULT 'udp' CHECK (transport IN ('udp', 'tcp', 'tls')),

    -- FreeSWITCH specific
    fs_profile VARCHAR(50), -- 'internal', 'external'
    fs_context VARCHAR(50), -- 'from-pbx', 'public'

    -- Kamailio specific
    kam_gwid INTEGER,
    kam_gw_type INTEGER DEFAULT 9, -- 8=carrier, 9=pbx

    -- Status
    enabled BOOLEAN DEFAULT true,
    is_primary BOOLEAN DEFAULT false,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(endpoint_type, ip_address, port)
);

-- Table for internal routes between endpoints
CREATE TABLE IF NOT EXISTS internal_routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,

    -- Route direction
    route_type VARCHAR(30) NOT NULL CHECK (route_type IN ('fs_to_kamailio', 'kamailio_to_fs')),

    -- Source endpoint
    source_endpoint_id UUID REFERENCES system_endpoints(id) ON DELETE CASCADE,

    -- Destination endpoint
    dest_endpoint_id UUID REFERENCES system_endpoints(id) ON DELETE CASCADE,

    -- Routing options
    bypass_media BOOLEAN DEFAULT true,
    inherit_codec BOOLEAN DEFAULT true,
    enable_100rel BOOLEAN DEFAULT true,
    call_timeout INTEGER DEFAULT 60,

    -- For FS->Kamailio: pattern matching
    prefix_pattern VARCHAR(50) DEFAULT '.*',

    -- Sync status
    sync_status VARCHAR(20) DEFAULT 'pending' CHECK (sync_status IN ('pending', 'synced', 'error')),
    sync_error TEXT,
    last_sync TIMESTAMPTZ,

    -- Status
    priority INTEGER DEFAULT 100,
    enabled BOOLEAN DEFAULT true,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Insert default endpoints based on current configuration
INSERT INTO system_endpoints (name, endpoint_type, description, ip_address, port, transport, fs_profile, fs_context, enabled, is_primary)
VALUES
    ('FreeSWITCH Internal', 'freeswitch', 'FreeSWITCH internal profile for registered devices', '10.10.22.4', 5080, 'udp', 'internal', 'from-pbx', true, true),
    ('FreeSWITCH External', 'freeswitch', 'FreeSWITCH external profile for trunk connections', '10.10.22.4', 5062, 'udp', 'external', 'public', true, false)
ON CONFLICT DO NOTHING;

INSERT INTO system_endpoints (name, endpoint_type, description, ip_address, port, transport, kam_gwid, kam_gw_type, enabled, is_primary)
VALUES
    ('Kamailio SBC', 'kamailio', 'Kamailio SBC for carrier routing', '10.10.22.4', 5060, 'udp', NULL, 8, true, true),
    ('Kamailio PBX Endpoint', 'kamailio', 'Kamailio endpoint receiving calls from FreeSWITCH', '10.10.22.4', 5082, 'udp', 100, 9, true, false)
ON CONFLICT DO NOTHING;

-- Insert default internal routes
INSERT INTO internal_routes (name, description, route_type, source_endpoint_id, dest_endpoint_id, bypass_media, inherit_codec, priority, enabled, sync_status)
SELECT
    'FreeSWITCH to Kamailio (Outbound)',
    'Route outbound calls from PBX to Kamailio SBC for carrier routing',
    'fs_to_kamailio',
    (SELECT id FROM system_endpoints WHERE name = 'FreeSWITCH External' LIMIT 1),
    (SELECT id FROM system_endpoints WHERE name = 'Kamailio SBC' LIMIT 1),
    true, true, 100, true, 'synced'
WHERE NOT EXISTS (SELECT 1 FROM internal_routes WHERE route_type = 'fs_to_kamailio')
ON CONFLICT DO NOTHING;

INSERT INTO internal_routes (name, description, route_type, source_endpoint_id, dest_endpoint_id, bypass_media, inherit_codec, priority, enabled, sync_status)
SELECT
    'Kamailio to FreeSWITCH (Inbound)',
    'Route inbound calls from carriers to FreeSWITCH PBX',
    'kamailio_to_fs',
    (SELECT id FROM system_endpoints WHERE name = 'Kamailio SBC' LIMIT 1),
    (SELECT id FROM system_endpoints WHERE name = 'FreeSWITCH Internal' LIMIT 1),
    true, true, 100, true, 'synced'
WHERE NOT EXISTS (SELECT 1 FROM internal_routes WHERE route_type = 'kamailio_to_fs')
ON CONFLICT DO NOTHING;

-- Indexes
CREATE INDEX IF NOT EXISTS idx_internal_routes_type ON internal_routes(route_type);
CREATE INDEX IF NOT EXISTS idx_internal_routes_enabled ON internal_routes(enabled);
CREATE INDEX IF NOT EXISTS idx_system_endpoints_type ON system_endpoints(endpoint_type);
