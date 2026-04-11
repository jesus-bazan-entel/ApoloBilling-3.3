-- Migration: Trunk Type and SIP Status
-- Date: 2026-03-22
-- Description: Agrega tipo de troncal (privada/pública) y monitoreo de estado SIP

-- ============================================
-- AGREGAR CAMPOS A routing_trunks
-- ============================================

-- Tipo de troncal: private (FreeSWITCH) o public (Kamailio)
ALTER TABLE routing_trunks
ADD COLUMN IF NOT EXISTS trunk_type VARCHAR(10) DEFAULT 'public'
CHECK (trunk_type IN ('private', 'public'));

-- Referencia a FreeSWITCH gateway (para troncales privadas)
ALTER TABLE routing_trunks
ADD COLUMN IF NOT EXISTS freeswitch_gateway_name VARCHAR(100);

-- Estado de conexión SIP
ALTER TABLE routing_trunks
ADD COLUMN IF NOT EXISTS sip_status VARCHAR(20) DEFAULT 'unknown'
CHECK (sip_status IN ('unknown', 'reachable', 'unreachable', 'checking'));

-- Último mensaje de estado
ALTER TABLE routing_trunks
ADD COLUMN IF NOT EXISTS sip_status_message TEXT;

-- Última verificación OPTIONS
ALTER TABLE routing_trunks
ADD COLUMN IF NOT EXISTS last_options_check TIMESTAMPTZ;

-- Tiempo de respuesta del último OPTIONS (ms)
ALTER TABLE routing_trunks
ADD COLUMN IF NOT EXISTS last_options_latency_ms INTEGER;

-- Código de respuesta del último OPTIONS
ALTER TABLE routing_trunks
ADD COLUMN IF NOT EXISTS last_options_response_code INTEGER;

-- Índices para optimización
CREATE INDEX IF NOT EXISTS idx_routing_trunks_trunk_type ON routing_trunks(trunk_type);
CREATE INDEX IF NOT EXISTS idx_routing_trunks_sip_status ON routing_trunks(sip_status);

-- ============================================
-- TABLA: routing_sip_status_log
-- Historial de verificaciones OPTIONS
-- ============================================

CREATE TABLE IF NOT EXISTS routing_sip_status_log (
    id BIGSERIAL PRIMARY KEY,
    trunk_id UUID NOT NULL REFERENCES routing_trunks(id) ON DELETE CASCADE,
    check_timestamp TIMESTAMPTZ DEFAULT NOW(),
    check_source VARCHAR(20) NOT NULL CHECK (check_source IN ('freeswitch', 'kamailio')),
    status VARCHAR(20) NOT NULL CHECK (status IN ('success', 'timeout', 'error', 'rejected')),
    response_code INTEGER,
    latency_ms INTEGER,
    -- Detalles del intercambio SIP
    request_sent TEXT,        -- Mensaje OPTIONS enviado
    response_received TEXT,   -- Respuesta recibida
    error_message TEXT,
    -- Información adicional del peer
    peer_user_agent VARCHAR(255),
    peer_allow_methods VARCHAR(255)
);

CREATE INDEX idx_routing_sip_status_log_trunk ON routing_sip_status_log(trunk_id);
CREATE INDEX idx_routing_sip_status_log_timestamp ON routing_sip_status_log(check_timestamp DESC);
CREATE INDEX idx_routing_sip_status_log_status ON routing_sip_status_log(status);

-- Partición por tiempo (mantener solo últimos 7 días)
-- Nota: Implementar limpieza via cron job

-- ============================================
-- COMENTARIOS
-- ============================================

COMMENT ON COLUMN routing_trunks.trunk_type IS 'private=red interna (FreeSWITCH), public=otros operadores (Kamailio)';
COMMENT ON COLUMN routing_trunks.freeswitch_gateway_name IS 'Nombre del gateway en FreeSWITCH (solo para trunk_type=private)';
COMMENT ON COLUMN routing_trunks.sip_status IS 'Estado de conexión SIP: unknown, reachable, unreachable, checking';
COMMENT ON COLUMN routing_trunks.sip_status_message IS 'Mensaje descriptivo del estado SIP actual';
COMMENT ON COLUMN routing_trunks.last_options_check IS 'Timestamp de la última verificación OPTIONS';
COMMENT ON COLUMN routing_trunks.last_options_latency_ms IS 'Latencia en ms de la última respuesta OPTIONS';
COMMENT ON COLUMN routing_trunks.last_options_response_code IS 'Código SIP de la última respuesta OPTIONS (200, 403, etc)';

COMMENT ON TABLE routing_sip_status_log IS 'Historial de verificaciones OPTIONS para cada troncal';
COMMENT ON COLUMN routing_sip_status_log.check_source IS 'Sistema que realizó la verificación: freeswitch o kamailio';
COMMENT ON COLUMN routing_sip_status_log.request_sent IS 'Mensaje OPTIONS SIP enviado (para debug)';
COMMENT ON COLUMN routing_sip_status_log.response_received IS 'Respuesta SIP recibida (para debug)';
