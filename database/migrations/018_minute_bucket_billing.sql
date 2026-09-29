-- Migration: Bolsas de minutos (Plan Control / Plan Abierto)
-- Date: 2026-09-24
-- Description: Introduce un modelo de facturacion por bolsa de minutos mensual,
--              coexistiendo con el modelo de saldo actual via accounts/plans.billing_model.
--              Ver /opt/ApoloBilling PRD "Bolsas de Minutos" para el diseno completo.
--
-- No destructiva: todas las cuentas/planes existentes quedan en billing_model='balance'
-- y todas las rate_cards existentes quedan en card_group='standard' (comportamiento
-- identico al actual). El bloque de INSERTs usa ON CONFLICT DO NOTHING.

BEGIN;

-- ============================================
-- 1. Extender plans
-- ============================================
ALTER TABLE plans ADD COLUMN IF NOT EXISTS billing_model VARCHAR(20) NOT NULL DEFAULT 'balance'
    CHECK (billing_model IN ('balance', 'minute_bucket'));
ALTER TABLE plans ADD COLUMN IF NOT EXISTS bucket_type VARCHAR(20)
    CHECK (bucket_type IN ('control', 'abierto'));
ALTER TABLE plans ADD COLUMN IF NOT EXISTS included_minutes INTEGER NOT NULL DEFAULT 0
    CHECK (included_minutes >= 0);
ALTER TABLE plans ADD COLUMN IF NOT EXISTS proration_mode VARCHAR(20) NOT NULL DEFAULT 'full'
    CHECK (proration_mode IN ('full', 'prorated'));
ALTER TABLE plans ADD COLUMN IF NOT EXISTS overage_flat_rate_per_minute NUMERIC(12,6);
ALTER TABLE plans ADD COLUMN IF NOT EXISTS overage_tariff_group VARCHAR(50);

COMMENT ON COLUMN plans.billing_model IS 'balance (saldo, comportamiento actual) o minute_bucket (bolsa de minutos)';
COMMENT ON COLUMN plans.bucket_type IS 'control (bloquea al agotar) o abierto (nunca bloquea, tarifica excedente) - solo aplica si billing_model=minute_bucket';
COMMENT ON COLUMN plans.included_minutes IS 'Minutos incluidos en la bolsa base mensual';
COMMENT ON COLUMN plans.proration_mode IS 'full = bolsa completa desde el alta; prorated = prorratea por dias restantes del mes en el primer ciclo';
COMMENT ON COLUMN plans.overage_flat_rate_per_minute IS 'Tarifa fija de excedente por minuto; si es NULL se usa overage_tariff_group (LPM)';
COMMENT ON COLUMN plans.overage_tariff_group IS 'rate_cards.card_group a usar para tasar excedente via LPM cuando no hay tarifa fija; NULL = usa el grupo standard';

-- ============================================
-- 2. Extender accounts
-- ============================================
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS billing_model VARCHAR(20) NOT NULL DEFAULT 'balance'
    CHECK (billing_model IN ('balance', 'minute_bucket'));
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS bucket_type VARCHAR(20)
    CHECK (bucket_type IN ('control', 'abierto'));
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS included_minutes INTEGER NOT NULL DEFAULT 0
    CHECK (included_minutes >= 0);
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS proration_mode VARCHAR(20) NOT NULL DEFAULT 'full'
    CHECK (proration_mode IN ('full', 'prorated'));
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS overage_flat_rate_per_minute NUMERIC(12,6);
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS overage_tariff_group VARCHAR(50);
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS notification_email VARCHAR(255);

COMMENT ON COLUMN accounts.notification_email IS 'Email de contacto del cliente para alertas de consumo de bolsa (80%/100%). NULL = no se envian alertas.';

-- ============================================
-- 3. Tarifario de excedente reutilizando rate_cards (LPM)
-- ============================================
ALTER TABLE rate_cards ADD COLUMN IF NOT EXISTS card_group VARCHAR(50) NOT NULL DEFAULT 'standard';
CREATE INDEX IF NOT EXISTS idx_rate_cards_group ON rate_cards(card_group);
COMMENT ON COLUMN rate_cards.card_group IS 'Agrupa tarifarios alternos (ej. overage_fibertel) sin duplicar el motor LPM; standard = tarifa normal de salida';

-- ============================================
-- 4. Ciclos de bolsa por cuenta (mes calendario, igual para todas las cuentas)
-- ============================================
CREATE TABLE IF NOT EXISTS minute_bucket_cycles (
    id SERIAL PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    cycle_start DATE NOT NULL,
    cycle_end   DATE NOT NULL,
    included_minutes   INTEGER NOT NULL,              -- valor EFECTIVO del ciclo (ya prorrateado y/o editado a mano)
    is_prorated         BOOLEAN NOT NULL DEFAULT false,
    proration_days      INTEGER,                       -- dias considerados si is_prorated=true (trazabilidad en reporte)
    additional_minutes  INTEGER NOT NULL DEFAULT 0,     -- suma denormalizada de minute_bucket_additions
    reserved_minutes    NUMERIC(10,2) NOT NULL DEFAULT 0,  -- llamadas en curso (soft-hold de concurrencia)
    consumed_minutes    NUMERIC(10,2) NOT NULL DEFAULT 0,  -- consumido real in-bucket (de CDRs cerrados)
    overage_minutes     NUMERIC(10,2) NOT NULL DEFAULT 0,  -- minutos excedentes acumulados
    overage_cost        NUMERIC(12,4) NOT NULL DEFAULT 0,  -- costo tarificado del excedente
    alert_80_sent_at    TIMESTAMP WITH TIME ZONE,          -- evita reenviar la alerta de 80% en el mismo ciclo
    alert_100_sent_at   TIMESTAMP WITH TIME ZONE,          -- evita reenviar la alerta de 100% en el mismo ciclo
    status VARCHAR(20) NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed')),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE (account_id, cycle_start)
);
CREATE INDEX IF NOT EXISTS idx_bucket_cycles_account_open ON minute_bucket_cycles(account_id) WHERE status = 'open';
CREATE INDEX IF NOT EXISTS idx_bucket_cycles_account ON minute_bucket_cycles(account_id, cycle_start DESC);

COMMENT ON TABLE minute_bucket_cycles IS 'Estado del ciclo mensual de bolsa de minutos por cuenta. Se abre de forma perezosa en el primer intento de llamada del mes.';

-- ============================================
-- 5. Adiciones de bolsa (bolsas adicionales contratadas dentro del ciclo)
-- ============================================
CREATE TABLE IF NOT EXISTS minute_bucket_additions (
    id SERIAL PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    cycle_id INTEGER NOT NULL REFERENCES minute_bucket_cycles(id) ON DELETE CASCADE,
    minutes_added INTEGER NOT NULL CHECK (minutes_added > 0),
    reason VARCHAR(255),
    created_by VARCHAR(100) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_bucket_additions_cycle ON minute_bucket_additions(cycle_id);

-- ============================================
-- 6. Detalle de bolsa por CDR (desglose in-bucket / overage para el reporte)
-- ============================================
ALTER TABLE cdrs ADD COLUMN IF NOT EXISTS bucket_cycle_id INTEGER REFERENCES minute_bucket_cycles(id);
ALTER TABLE cdrs ADD COLUMN IF NOT EXISTS in_bucket_minutes NUMERIC(10,2);
ALTER TABLE cdrs ADD COLUMN IF NOT EXISTS overage_minutes NUMERIC(10,2);
ALTER TABLE cdrs ADD COLUMN IF NOT EXISTS overage_cost NUMERIC(12,4);
ALTER TABLE cdrs ADD COLUMN IF NOT EXISTS overage_rate_applied NUMERIC(12,6);
CREATE INDEX IF NOT EXISTS idx_cdrs_bucket_cycle ON cdrs(bucket_cycle_id) WHERE bucket_cycle_id IS NOT NULL;

-- ============================================
-- 7. Alertas de consumo (80% / 100%) por email
-- ============================================
-- El motor de facturacion en tiempo real (rust-billing-engine) solo ENCOLA la alerta
-- (INSERT aqui) al cruzar el umbral; el envio real de SMTP lo hace un job en
-- rust-backend para no bloquear el hot path de autorizacion/CDR. Credenciales SMTP
-- via variables de entorno (SMTP_*), nunca en una tabla expuesta por /api/v1/settings.
CREATE TABLE IF NOT EXISTS minute_bucket_alerts (
    id SERIAL PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    cycle_id INTEGER NOT NULL REFERENCES minute_bucket_cycles(id) ON DELETE CASCADE,
    threshold_pct SMALLINT NOT NULL CHECK (threshold_pct IN (80, 100)),
    status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'sent', 'failed')),
    error_message TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    sent_at TIMESTAMP WITH TIME ZONE,
    UNIQUE (cycle_id, threshold_pct)
);
CREATE INDEX IF NOT EXISTS idx_bucket_alerts_pending ON minute_bucket_alerts(status) WHERE status = 'pending';

-- ============================================
-- 8. Modulo de menu para el backoffice
-- ============================================
INSERT INTO system_modules (module_key, nombre, seccion, min_role, always_visible, orden) VALUES
    ('minute_buckets', 'Bolsas de Minutos', 'Facturacion', 'usuario', false, 65)
ON CONFLICT (module_key) DO NOTHING;

-- ============================================
-- 9. Permisos para el rol de aplicacion
-- ============================================
-- plans/accounts/rate_cards/cdrs son propiedad del rol 'postgres' (igual que el resto
-- del esquema legado) y apolo_user ya tenia SELECT/UPDATE via GRANT previos, por lo que
-- los ALTER TABLE de arriba no requieren grants nuevos. Las tablas NUEVAS si son creadas
-- por quien corre esta migracion, asi que se otorgan explicitamente al rol de la app.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'apolo_user') THEN
        GRANT SELECT, INSERT, UPDATE, DELETE
            ON minute_bucket_cycles, minute_bucket_additions, minute_bucket_alerts
            TO apolo_user;
        GRANT USAGE, SELECT ON SEQUENCE
            minute_bucket_cycles_id_seq, minute_bucket_additions_id_seq, minute_bucket_alerts_id_seq
            TO apolo_user;
    END IF;
END $$;

COMMIT;
