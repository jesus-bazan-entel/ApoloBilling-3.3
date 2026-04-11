-- Migration: Add SIP devices management with mod_xml_curl integration
-- Date: 2026-03-21
-- Description: Sistema de gestión de dispositivos SIP para FreeSWITCH

-- ============================================
-- TABLA: sip_devices
-- Dispositivos SIP (teléfonos IP, softphones, etc.)
-- ============================================

CREATE TABLE IF NOT EXISTS sip_devices (
    id SERIAL PRIMARY KEY,

    -- Relación con cuenta de facturación
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,

    -- Credenciales SIP (únicas por dominio)
    sip_username VARCHAR(64) NOT NULL,
    sip_domain VARCHAR(255) NOT NULL DEFAULT 'apolo.local',

    -- Contraseña encriptada con AES-256-GCM
    password_encrypted BYTEA NOT NULL,
    password_nonce BYTEA NOT NULL,

    -- Hash A1 para autenticación digest SIP: MD5(username:realm:password)
    a1_hash VARCHAR(32) NOT NULL,

    -- Información del dispositivo
    display_name VARCHAR(100),
    description TEXT,

    -- Configuración de FreeSWITCH
    context VARCHAR(50) NOT NULL DEFAULT 'from-pbx',
    accountcode VARCHAR(50),  -- Enlaza con accounts.account_number para CDR

    -- Codecs permitidos (lista separada por comas)
    codecs VARCHAR(255) DEFAULT 'PCMU,PCMA,G729,opus',

    -- Límites
    max_registrations INTEGER NOT NULL DEFAULT 3,

    -- Estado
    enabled BOOLEAN NOT NULL DEFAULT true,

    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT unique_sip_user_domain UNIQUE (sip_username, sip_domain)
);

-- Índices para búsquedas frecuentes
CREATE INDEX idx_sip_devices_account ON sip_devices(account_id);
CREATE INDEX idx_sip_devices_username ON sip_devices(sip_username);
CREATE INDEX idx_sip_devices_domain ON sip_devices(sip_domain);
CREATE INDEX idx_sip_devices_enabled ON sip_devices(enabled) WHERE enabled = true;
CREATE INDEX idx_sip_devices_accountcode ON sip_devices(accountcode);

-- ============================================
-- TABLA: freeswitch_allowed_ips
-- IPs autorizadas para consultar mod_xml_curl
-- ============================================

CREATE TABLE IF NOT EXISTS freeswitch_allowed_ips (
    id SERIAL PRIMARY KEY,
    ip_address INET NOT NULL UNIQUE,
    description VARCHAR(255),
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_freeswitch_ips_enabled ON freeswitch_allowed_ips(enabled) WHERE enabled = true;

-- ============================================
-- DATOS INICIALES: IPs permitidas por defecto
-- ============================================

INSERT INTO freeswitch_allowed_ips (ip_address, description, enabled) VALUES
    ('127.0.0.1', 'Localhost IPv4', true),
    ('::1', 'Localhost IPv6', true)
ON CONFLICT (ip_address) DO NOTHING;

-- ============================================
-- FUNCIÓN: Actualizar updated_at automáticamente
-- ============================================

CREATE OR REPLACE FUNCTION update_sip_devices_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_sip_devices_updated_at
    BEFORE UPDATE ON sip_devices
    FOR EACH ROW
    EXECUTE FUNCTION update_sip_devices_updated_at();

CREATE TRIGGER trigger_freeswitch_ips_updated_at
    BEFORE UPDATE ON freeswitch_allowed_ips
    FOR EACH ROW
    EXECUTE FUNCTION update_sip_devices_updated_at();

-- ============================================
-- COMENTARIOS
-- ============================================

COMMENT ON TABLE sip_devices IS 'Dispositivos SIP registrados en el sistema, autenticados via mod_xml_curl';
COMMENT ON COLUMN sip_devices.sip_username IS 'Usuario SIP (extensión o nombre de usuario)';
COMMENT ON COLUMN sip_devices.sip_domain IS 'Dominio SIP (realm para autenticación)';
COMMENT ON COLUMN sip_devices.password_encrypted IS 'Contraseña encriptada con AES-256-GCM';
COMMENT ON COLUMN sip_devices.password_nonce IS 'Nonce usado para encriptación AES-256-GCM';
COMMENT ON COLUMN sip_devices.a1_hash IS 'Hash MD5(username:realm:password) para autenticación digest';
COMMENT ON COLUMN sip_devices.context IS 'Contexto de dialplan FreeSWITCH (from-pbx, from-internal, etc.)';
COMMENT ON COLUMN sip_devices.accountcode IS 'Código de cuenta para vincular CDRs con facturación';
COMMENT ON COLUMN sip_devices.codecs IS 'Lista de codecs permitidos separados por coma';
COMMENT ON COLUMN sip_devices.max_registrations IS 'Número máximo de registros simultáneos permitidos';

COMMENT ON TABLE freeswitch_allowed_ips IS 'IPs autorizadas para consultar el endpoint mod_xml_curl';
COMMENT ON COLUMN freeswitch_allowed_ips.ip_address IS 'Dirección IP del servidor FreeSWITCH';
