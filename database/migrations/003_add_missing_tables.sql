-- Migration: Add missing tables required by the application
-- Date: 2026-04-10
-- Description: Creates usuarios, active_calls, audit_logs, system_settings,
--              zones, prefixes, rate_zones tables and adds missing columns to accounts

-- ============================================
-- TABLA: usuarios (autenticación y usuarios del sistema)
-- ============================================

CREATE TABLE IF NOT EXISTS usuarios (
    id SERIAL PRIMARY KEY,
    username VARCHAR(100) UNIQUE NOT NULL,
    password VARCHAR(255) NOT NULL,
    nombre VARCHAR(100),
    apellido VARCHAR(100),
    email VARCHAR(100),
    role VARCHAR(20) NOT NULL DEFAULT 'operator',
    activo BOOLEAN DEFAULT true,
    ultimo_login TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_usuarios_username ON usuarios(username);
CREATE INDEX IF NOT EXISTS idx_usuarios_role ON usuarios(role);

-- ============================================
-- TABLA: active_calls (llamadas activas en tiempo real)
-- ============================================

CREATE TABLE IF NOT EXISTS active_calls (
    id SERIAL PRIMARY KEY,
    call_id VARCHAR(100) UNIQUE NOT NULL,
    calling_number VARCHAR(50),
    called_number VARCHAR(50),
    direction VARCHAR(20),
    start_time TIMESTAMP WITH TIME ZONE NOT NULL,
    answer_time TIMESTAMP WITH TIME ZONE,
    current_duration INTEGER DEFAULT 0,
    current_cost DECIMAL(12, 4) DEFAULT 0,
    connection_id VARCHAR(100),
    server VARCHAR(100),
    last_updated TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    status VARCHAR(20),
    client_id INTEGER,
    rate_per_minute DECIMAL(10, 6)
);

CREATE INDEX IF NOT EXISTS idx_active_calls_call_id ON active_calls(call_id);
CREATE INDEX IF NOT EXISTS idx_active_calls_client_id ON active_calls(client_id);
CREATE INDEX IF NOT EXISTS idx_active_calls_status ON active_calls(status);

-- ============================================
-- TABLA: audit_logs (auditoría del sistema)
-- ============================================

CREATE TABLE IF NOT EXISTS audit_logs (
    id BIGSERIAL PRIMARY KEY,
    user_id INTEGER,
    username VARCHAR(100) NOT NULL,
    action VARCHAR(100) NOT NULL,
    entity_type VARCHAR(100) NOT NULL,
    entity_id VARCHAR(255),
    details JSONB,
    ip_address VARCHAR(50),
    user_agent TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_username ON audit_logs(username);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs(action);
CREATE INDEX IF NOT EXISTS idx_audit_logs_entity ON audit_logs(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_created ON audit_logs(created_at);

-- ============================================
-- TABLA: system_settings (configuración del sistema)
-- ============================================

CREATE TABLE IF NOT EXISTS system_settings (
    key VARCHAR(255) PRIMARY KEY,
    value TEXT NOT NULL,
    description TEXT,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Configuración por defecto
INSERT INTO system_settings (key, value, description) VALUES
('outbound_authorization_enabled', 'true', 'Habilitar/deshabilitar autorización de llamadas salientes')
ON CONFLICT (key) DO NOTHING;

-- ============================================
-- TABLA: zones (zonas geográficas para tarifas)
-- ============================================

CREATE TABLE IF NOT EXISTS zones (
    id SERIAL PRIMARY KEY,
    country_id INTEGER,
    zone_name VARCHAR(200) NOT NULL UNIQUE,
    zone_code VARCHAR(50),
    description TEXT,
    zone_type VARCHAR(50) DEFAULT 'GEOGRAPHIC',
    region_name VARCHAR(100),
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_zones_name ON zones(zone_name);
CREATE INDEX IF NOT EXISTS idx_zones_enabled ON zones(enabled);

-- ============================================
-- TABLA: prefixes (prefijos telefónicos por zona)
-- ============================================

CREATE TABLE IF NOT EXISTS prefixes (
    id SERIAL PRIMARY KEY,
    zone_id INTEGER NOT NULL REFERENCES zones(id) ON DELETE CASCADE,
    prefix VARCHAR(20) NOT NULL UNIQUE,
    prefix_length INTEGER,
    operator_name VARCHAR(100),
    network_type VARCHAR(20) DEFAULT 'FIXED',
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_prefixes_zone ON prefixes(zone_id);
CREATE INDEX IF NOT EXISTS idx_prefixes_prefix ON prefixes(prefix);

-- ============================================
-- TABLA: rate_zones (tarifas por zona)
-- ============================================

CREATE TABLE IF NOT EXISTS rate_zones (
    id SERIAL PRIMARY KEY,
    zone_id INTEGER NOT NULL REFERENCES zones(id) ON DELETE CASCADE,
    rate_name VARCHAR(255),
    rate_per_minute DECIMAL(10, 6) NOT NULL,
    rate_per_call DECIMAL(10, 6) DEFAULT 0,
    billing_increment INTEGER DEFAULT 6,
    min_duration INTEGER DEFAULT 0,
    effective_from TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    currency VARCHAR(3) DEFAULT 'USD',
    priority INTEGER DEFAULT 0,
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_rate_zones_zone ON rate_zones(zone_id);
CREATE INDEX IF NOT EXISTS idx_rate_zones_enabled ON rate_zones(enabled);

-- ============================================
-- MODIFICAR TABLA: accounts (columnas faltantes)
-- ============================================

ALTER TABLE accounts ADD COLUMN IF NOT EXISTS max_concurrent_calls INTEGER DEFAULT 1;

-- Actualizar timestamps a WITH TIME ZONE si no lo son
ALTER TABLE accounts ALTER COLUMN created_at TYPE TIMESTAMP WITH TIME ZONE USING created_at AT TIME ZONE 'UTC';
ALTER TABLE accounts ALTER COLUMN updated_at TYPE TIMESTAMP WITH TIME ZONE USING updated_at AT TIME ZONE 'UTC';

-- ============================================
-- Trigger updated_at para nuevas tablas
-- ============================================

DROP TRIGGER IF EXISTS update_usuarios_updated_at ON usuarios;
CREATE TRIGGER update_usuarios_updated_at
    BEFORE UPDATE ON usuarios
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================
-- PERMISOS
-- ============================================

GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO apolo_user;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO apolo_user;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO apolo_user;
