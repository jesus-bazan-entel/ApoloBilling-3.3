-- ============================================
-- Migration 020: Unified SIP Endpoints
-- Merges sip_devices (credential-authenticated extensions) and endpoints
-- (IP-trusted PBX trunks, dr_gateways type=9) into a single carrier-class
-- model with two orthogonal axes:
--   endpoint_type: extension (single subscriber) | trunk (PBX/gateway)
--   auth_mode:     credentials (SIP REGISTER, digest)  | ip (trusted source IP)
--
-- auth_mode = 'ip' only makes sense for a whole PBX (trunk), not a single
-- extension, so that combination is rejected by a CHECK constraint.
-- auth_mode = 'credentials' never touches Kamailio, regardless of
-- endpoint_type - a credential-authenticated trunk is mechanically identical
-- to an extension on the FreeSWITCH side (mod_xml_curl + A1 hash), it's just
-- labeled/sized differently. Only auth_mode = 'ip' is synced to Kamailio's
-- dr_gateways (type=9) + address ACL (grp=9), exactly like the old
-- `endpoints` table.
-- ============================================

BEGIN;

CREATE TABLE IF NOT EXISTS sip_endpoints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,

    endpoint_type VARCHAR(20) NOT NULL CHECK (endpoint_type IN ('extension', 'trunk')),
    auth_mode VARCHAR(20) NOT NULL CHECK (auth_mode IN ('credentials', 'ip')),

    account_id INTEGER REFERENCES accounts(id),

    -- auth_mode = 'credentials' (SIP REGISTER, FreeSWITCH mod_xml_curl)
    sip_username VARCHAR(64),
    sip_domain VARCHAR(255) DEFAULT 'apolo.local',
    password_encrypted BYTEA,
    password_nonce BYTEA,
    a1_hash VARCHAR(32),
    max_registrations INTEGER DEFAULT 3,

    -- auth_mode = 'ip' (trusted source IP, Kamailio dr_gateways type=9)
    host VARCHAR(255),
    port INTEGER DEFAULT 5060,
    transport VARCHAR(10) DEFAULT 'udp' CHECK (transport IN ('udp', 'tcp', 'tls')),
    kamailio_gwid INTEGER,
    sync_status VARCHAR(20) DEFAULT 'pending' CHECK (sync_status IN ('pending', 'synced', 'error')),
    sync_error TEXT,
    is_default BOOLEAN DEFAULT false,

    -- Derived server-side from endpoint_type, never exposed to the user
    context VARCHAR(50) NOT NULL DEFAULT 'from-pbx',
    fs_profile VARCHAR(50) NOT NULL DEFAULT 'internal',

    display_name VARCHAR(100),
    accountcode VARCHAR(50),
    codecs VARCHAR(255) DEFAULT 'PCMU,PCMA,G729,opus',
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT chk_ip_auth_is_trunk CHECK (auth_mode <> 'ip' OR endpoint_type = 'trunk'),
    CONSTRAINT chk_credentials_require_username CHECK (
        auth_mode <> 'credentials' OR (sip_username IS NOT NULL AND a1_hash IS NOT NULL)
    ),
    CONSTRAINT chk_ip_requires_host CHECK (auth_mode <> 'ip' OR host IS NOT NULL)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_sip_endpoints_user_domain
    ON sip_endpoints (sip_username, sip_domain) WHERE sip_username IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_sip_endpoints_single_default
    ON sip_endpoints (is_default) WHERE is_default = true;

CREATE INDEX IF NOT EXISTS idx_sip_endpoints_account ON sip_endpoints (account_id);

-- ============================================
-- Backfill from sip_devices (-> extension/credentials)
-- ============================================
INSERT INTO sip_endpoints (
    name, description, endpoint_type, auth_mode, account_id,
    sip_username, sip_domain, password_encrypted, password_nonce, a1_hash,
    max_registrations, context, display_name, accountcode, codecs, enabled,
    created_at, updated_at
)
SELECT
    sip_username || '@' || sip_domain,
    description,
    'extension', 'credentials', account_id,
    sip_username, sip_domain, password_encrypted, password_nonce, a1_hash,
    max_registrations, context, display_name, accountcode, codecs, enabled,
    created_at, updated_at
FROM sip_devices
ON CONFLICT (name) DO NOTHING;

-- ============================================
-- Backfill from endpoints (-> trunk/ip)
-- ============================================
INSERT INTO sip_endpoints (
    name, description, endpoint_type, auth_mode,
    host, port, transport, fs_profile, kamailio_gwid, sync_status, sync_error,
    enabled, is_default, created_at, updated_at
)
SELECT
    name, description, 'trunk', 'ip',
    host, port, transport, fs_profile, kamailio_gwid, sync_status, sync_error,
    enabled, is_default, created_at, updated_at
FROM endpoints
ON CONFLICT (name) DO NOTHING;

-- Keep the old tables as a rollback-safe backup instead of dropping them.
ALTER TABLE IF EXISTS sip_devices RENAME TO sip_devices_legacy;
ALTER TABLE IF EXISTS endpoints RENAME TO endpoints_legacy;

-- ============================================
-- Module catalog: fold 'endpoints' module into 'sip_devices' (now the
-- unified SIP Endpoints module). Preserve existing per-user grants.
-- ============================================
UPDATE system_modules SET nombre = 'Dispositivos SIP' WHERE module_key = 'sip_devices';

INSERT INTO usuario_modulos (usuario_id, module_key, otorgado_por)
SELECT usuario_id, 'sip_devices', otorgado_por
FROM usuario_modulos
WHERE module_key = 'endpoints'
ON CONFLICT (usuario_id, module_key) DO NOTHING;

DELETE FROM system_modules WHERE module_key = 'endpoints';

COMMIT;
