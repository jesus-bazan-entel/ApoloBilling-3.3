-- Migration: Add plans table and plan_id to accounts
-- Date: 2026-01-24
-- Description: Sistema de planes para estandarizar creación de cuentas

-- ============================================
-- TABLA: plans
-- ============================================

CREATE TABLE IF NOT EXISTS plans (
    id SERIAL PRIMARY KEY,
    plan_name VARCHAR(100) NOT NULL,
    plan_code VARCHAR(50) UNIQUE NOT NULL,
    account_type VARCHAR(20) NOT NULL CHECK (account_type IN ('PREPAID', 'POSTPAID')),

    -- Valores que se aplicarán automáticamente al crear cuenta
    initial_balance DECIMAL(12, 4) NOT NULL DEFAULT 0.0000,
    credit_limit DECIMAL(12, 4) NOT NULL DEFAULT 0.0000,
    max_concurrent_calls INTEGER NOT NULL DEFAULT 5,

    -- Metadata
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_by VARCHAR(100) DEFAULT 'system'
);

CREATE INDEX idx_plans_type ON plans(account_type);
CREATE INDEX idx_plans_enabled ON plans(enabled);
CREATE INDEX idx_plans_code ON plans(plan_code);

-- ============================================
-- MODIFICAR TABLA: accounts
-- ============================================

-- Agregar columna plan_id (nullable para cuentas existentes)
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS plan_id INTEGER REFERENCES plans(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_accounts_plan ON accounts(plan_id);

-- ============================================
-- DATOS INICIALES: Planes por defecto
-- ============================================

INSERT INTO plans (plan_name, plan_code, account_type, initial_balance, credit_limit, max_concurrent_calls, description, enabled) VALUES
-- PREPAGO (initial_balance DEBE ser > 0)
('Prepago Básico', 'PRE-BAS', 'PREPAID', 10.00, 0.00, 5, 'Plan básico con S/10 de bono inicial', true),
('Prepago Plus', 'PRE-PLUS', 'PREPAID', 50.00, 0.00, 10, 'Plan con bono de bienvenida de S/50', true),

-- POSTPAGO
('Postpago 100', 'POST-100', 'POSTPAID', 0.00, 100.00, 5, 'Límite de consumo mensual: S/100', true),
('Postpago 300', 'POST-300', 'POSTPAID', 0.00, 300.00, 10, 'Límite de consumo mensual: S/300', true),
('Postpago 500', 'POST-500', 'POSTPAID', 0.00, 500.00, 15, 'Límite de consumo mensual: S/500', true),
('Postpago 1000', 'POST-1K', 'POSTPAID', 0.00, 1000.00, 20, 'Límite de consumo mensual: S/1,000', true),
('Postpago 5000', 'POST-5K', 'POSTPAID', 0.00, 5000.00, 50, 'Plan corporativo - Límite: S/5,000', true)
ON CONFLICT (plan_code) DO NOTHING;

-- ============================================
-- COMENTARIOS
-- ============================================

COMMENT ON TABLE plans IS 'Planes predefinidos para creación rápida de cuentas';
COMMENT ON COLUMN plans.plan_code IS 'Código único del plan (ej: PRE-BAS, POST-500)';
COMMENT ON COLUMN plans.initial_balance IS 'Saldo inicial que se otorga al crear la cuenta';
COMMENT ON COLUMN plans.credit_limit IS 'Límite de crédito para cuentas postpago';
COMMENT ON COLUMN accounts.plan_id IS 'Plan usado al crear la cuenta (nullable para cuentas existentes)';
