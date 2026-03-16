-- =============================================================================
-- APOLO BILLING ENGINE - DATABASE SCHEMA
-- =============================================================================

-- Conectar a la base de datos correcta
\c apolo_billing;

-- Limpiar si existe (CUIDADO: esto borra todo)
DROP TABLE IF EXISTS balance_transactions CASCADE;
DROP TABLE IF EXISTS call_detail_records CASCADE;
DROP TABLE IF EXISTS balance_reservations CASCADE;
DROP TABLE IF EXISTS rate_cards CASCADE;
DROP TABLE IF EXISTS accounts CASCADE;
DROP TYPE IF EXISTS account_type CASCADE;
DROP TYPE IF EXISTS account_status CASCADE;
DROP TYPE IF EXISTS reservation_status CASCADE;
DROP TYPE IF EXISTS reservation_type CASCADE;
DROP TYPE IF EXISTS transaction_type CASCADE;

-- =============================================================================
-- TIPOS ENUM
-- =============================================================================

-- Tipo de cuenta
CREATE TYPE account_type AS ENUM (
    'PREPAID',
    'POSTPAID'
);

-- Estado de cuenta
CREATE TYPE account_status AS ENUM (
    'ACTIVE',
    'SUSPENDED',
    'CLOSED'
);

-- Estado de reservación
CREATE TYPE reservation_status AS ENUM (
    'active',
    'partially_consumed',
    'fully_consumed',
    'released',
    'expired'
);

-- Tipo de reservación
CREATE TYPE reservation_type AS ENUM (
    'initial',
    'extension',
    'adjustment'
);

-- Tipo de transacción
CREATE TYPE transaction_type AS ENUM (
    'credit',
    'debit',
    'reservation_create',
    'reservation_consume',
    'reservation_release',
    'adjustment',
    'refund'
);

-- =============================================================================
-- TABLA: accounts
-- =============================================================================
CREATE TABLE accounts (
    id SERIAL PRIMARY KEY,
    account_code VARCHAR(50) UNIQUE NOT NULL,
    account_name VARCHAR(200) NOT NULL,
    account_type account_type NOT NULL DEFAULT 'PREPAID',
    status account_status NOT NULL DEFAULT 'ACTIVE',
    balance DECIMAL(12, 4) NOT NULL DEFAULT 0.0000,
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    credit_limit DECIMAL(12, 4) DEFAULT 0.0000,
    
    -- Metadata
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_by VARCHAR(100) DEFAULT 'system',
    updated_by VARCHAR(100) DEFAULT 'system',
    
    -- Constraints
    CONSTRAINT chk_balance_positive_prepaid 
        CHECK (account_type = 'POSTPAID' OR balance >= 0),
    CONSTRAINT chk_credit_limit_postpaid 
        CHECK (account_type = 'PREPAID' OR credit_limit >= 0)
);

-- Índices
CREATE INDEX idx_accounts_code ON accounts(account_code);
CREATE INDEX idx_accounts_status ON accounts(status);
CREATE INDEX idx_accounts_type ON accounts(account_type);

-- Datos de prueba
INSERT INTO accounts (id, account_code, account_name, account_type, status, balance, created_by) VALUES
(1, '100001', 'Cliente Prepago Demo', 'PREPAID', 'ACTIVE', 10.0000, 'system'),
(2, '100002', 'Cliente Postpago Demo', 'POSTPAID', 'ACTIVE', 0.0000, 'system'),
(3, '100001', 'Test Account Prepaid', 'PREPAID', 'ACTIVE', 10.0000, 'system')
ON CONFLICT (account_code) DO NOTHING;

-- =============================================================================
-- TABLA: rate_cards
-- =============================================================================
CREATE TABLE rate_cards (
    id SERIAL PRIMARY KEY,
    rate_name VARCHAR(200) NOT NULL,
    destination_prefix VARCHAR(20) NOT NULL,
    destination_name VARCHAR(200),
    rate_per_minute DECIMAL(10, 6) NOT NULL,
    rate_increment_seconds INTEGER NOT NULL DEFAULT 6,
    initial_increment_seconds INTEGER NOT NULL DEFAULT 6,
    priority INTEGER NOT NULL DEFAULT 100,
    
    -- Vigencia
    effective_date DATE NOT NULL DEFAULT CURRENT_DATE,
    expiry_date DATE,
    
    -- Metadata
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_by VARCHAR(100) DEFAULT 'system',
    
    -- Constraints
    CONSTRAINT chk_rate_positive CHECK (rate_per_minute >= 0),
    CONSTRAINT chk_increment_positive CHECK (rate_increment_seconds > 0),
    CONSTRAINT chk_dates CHECK (expiry_date IS NULL OR expiry_date >= effective_date)
);

-- Índices
CREATE INDEX idx_rate_cards_prefix ON rate_cards(destination_prefix);
CREATE INDEX idx_rate_cards_priority ON rate_cards(priority DESC);
CREATE INDEX idx_rate_cards_dates ON rate_cards(effective_date, expiry_date);
CREATE INDEX idx_rate_cards_prefix_priority ON rate_cards(destination_prefix, priority DESC);

-- Datos de prueba: Rate Cards para Perú
INSERT INTO rate_cards (rate_name, destination_prefix, destination_name, rate_per_minute, rate_increment_seconds, priority, created_by) VALUES
-- Perú - Móviles
('Perú Móvil', '519', 'Perú - Móvil', 0.0180, 6, 150, 'system'),
('Perú Móvil Claro', '51987', 'Perú - Claro Móvil', 0.0175, 6, 200, 'system'),
('Perú Móvil Movistar', '51997', 'Perú - Movistar Móvil', 0.0175, 6, 200, 'system'),
('Perú Móvil Entel', '51967', 'Perú - Entel Móvil', 0.0180, 6, 200, 'system'),
('Perú Móvil Bitel', '51939', 'Perú - Bitel Móvil', 0.0185, 6, 200, 'system'),

-- Perú - Fijos por ciudad
('Perú Lima Fijo', '511', 'Perú - Lima Fijo', 0.0120, 6, 100, 'system'),
('Perú Arequipa Fijo', '5154', 'Perú - Arequipa Fijo', 0.0140, 6, 100, 'system'),
('Perú Cusco Fijo', '5184', 'Perú - Cusco Fijo', 0.0140, 6, 100, 'system'),
('Perú Trujillo Fijo', '5144', 'Perú - Trujillo Fijo', 0.0140, 6, 100, 'system'),

-- Perú - Nacional (fallback)
('Perú Nacional', '51', 'Perú - Nacional', 0.0150, 6, 50, 'system')

ON CONFLICT DO NOTHING;

-- =============================================================================
-- TABLA: balance_reservations
-- =============================================================================
CREATE TABLE balance_reservations (
    id UUID PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    call_uuid VARCHAR(100) NOT NULL,
    
    -- Montos
    reserved_amount DECIMAL(12, 4) NOT NULL DEFAULT 0.0000,
    consumed_amount DECIMAL(12, 4) NOT NULL DEFAULT 0.0000,
    released_amount DECIMAL(12, 4) NOT NULL DEFAULT 0.0000,
    
    -- Estado y tipo
    status reservation_status NOT NULL DEFAULT 'active',
    reservation_type reservation_type NOT NULL DEFAULT 'initial',
    
    -- Detalles de rating
    destination_prefix VARCHAR(20),
    rate_per_minute DECIMAL(10, 6),
    reserved_minutes INTEGER,
    
    -- Expiración
    expires_at TIMESTAMP NOT NULL,
    
    -- Timestamps
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    consumed_at TIMESTAMP,
    released_at TIMESTAMP,
    
    -- Metadata
    created_by VARCHAR(100) DEFAULT 'system',
    updated_by VARCHAR(100) DEFAULT 'system',
    
    -- Constraints
    CONSTRAINT chk_amounts_positive CHECK (
        reserved_amount >= 0 AND 
        consumed_amount >= 0 AND 
        released_amount >= 0
    ),
    CONSTRAINT chk_consumed_released_sum CHECK (
        consumed_amount + released_amount <= reserved_amount
    )
);

-- Índices
CREATE INDEX idx_reservations_account ON balance_reservations(account_id);
CREATE INDEX idx_reservations_call ON balance_reservations(call_uuid);
CREATE INDEX idx_reservations_status ON balance_reservations(status);
CREATE INDEX idx_reservations_expires ON balance_reservations(expires_at);
CREATE INDEX idx_reservations_account_status ON balance_reservations(account_id, status);

-- =============================================================================
-- TABLA: balance_transactions
-- =============================================================================
CREATE TABLE balance_transactions (
    id BIGSERIAL PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    
    -- Monto y balance
    amount DECIMAL(12, 4) NOT NULL,
    previous_balance DECIMAL(12, 4) NOT NULL,
    new_balance DECIMAL(12, 4) NOT NULL,
    
    -- Tipo y razón
    transaction_type transaction_type NOT NULL,
    reason TEXT,
    
    -- Relaciones
    call_uuid VARCHAR(100),
    reservation_id UUID REFERENCES balance_reservations(id) ON DELETE SET NULL,
    
    -- Metadata
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_by VARCHAR(100) DEFAULT 'system',
    
    -- Constraints
    CONSTRAINT chk_balance_change CHECK (new_balance = previous_balance + amount)
);

-- Índices
CREATE INDEX idx_transactions_account ON balance_transactions(account_id);
CREATE INDEX idx_transactions_call ON balance_transactions(call_uuid);
CREATE INDEX idx_transactions_type ON balance_transactions(transaction_type);
CREATE INDEX idx_transactions_created ON balance_transactions(created_at);
CREATE INDEX idx_transactions_reservation ON balance_transactions(reservation_id);

-- =============================================================================
-- TABLA: call_detail_records (CDR)
-- =============================================================================
CREATE TABLE call_detail_records (
    id BIGSERIAL PRIMARY KEY,
    
    -- Identificación de la llamada
    call_uuid VARCHAR(100) UNIQUE NOT NULL,
    account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
    
    -- Origen y destino
    caller_number VARCHAR(50) NOT NULL,
    callee_number VARCHAR(50) NOT NULL,
    destination_prefix VARCHAR(20),
    
    -- Tiempos (en segundos)
    start_time TIMESTAMP NOT NULL,
    answer_time TIMESTAMP,
    end_time TIMESTAMP NOT NULL,
    duration INTEGER NOT NULL DEFAULT 0,
    billsec INTEGER NOT NULL DEFAULT 0,
    
    -- Rating
    rate_card_id INTEGER REFERENCES rate_cards(id) ON DELETE SET NULL,
    rate_per_minute DECIMAL(10, 6),
    cost DECIMAL(12, 4) DEFAULT 0.0000,
    
    -- Hangup
    hangup_cause VARCHAR(50),
    hangup_disposition VARCHAR(20),
    
    -- Reservación relacionada
    reservation_id UUID REFERENCES balance_reservations(id) ON DELETE SET NULL,
    
    -- Metadata
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMP,
    
    -- Constraints
    CONSTRAINT chk_cdr_times CHECK (end_time >= start_time),
    CONSTRAINT chk_cdr_duration CHECK (duration >= 0),
    CONSTRAINT chk_cdr_billsec CHECK (billsec >= 0 AND billsec <= duration)
);

-- Índices
CREATE INDEX idx_cdr_uuid ON call_detail_records(call_uuid);
CREATE INDEX idx_cdr_account ON call_detail_records(account_id);
CREATE INDEX idx_cdr_caller ON call_detail_records(caller_number);
CREATE INDEX idx_cdr_callee ON call_detail_records(callee_number);
CREATE INDEX idx_cdr_start_time ON call_detail_records(start_time);
CREATE INDEX idx_cdr_account_start ON call_detail_records(account_id, start_time);
CREATE INDEX idx_cdr_reservation ON call_detail_records(reservation_id);

-- =============================================================================
-- FUNCIONES Y TRIGGERS
-- =============================================================================

-- Función para actualizar updated_at automáticamente
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Triggers para updated_at
CREATE TRIGGER update_accounts_updated_at
    BEFORE UPDATE ON accounts
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_reservations_updated_at
    BEFORE UPDATE ON balance_reservations
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_rate_cards_updated_at
    BEFORE UPDATE ON rate_cards
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- =============================================================================
-- VISTAS ÚTILES
-- =============================================================================

-- Vista de balance disponible por cuenta
CREATE OR REPLACE VIEW v_available_balance AS
SELECT 
    a.id,
    a.account_code,
    a.account_name,
    a.balance,
    COALESCE(SUM(br.reserved_amount - br.consumed_amount), 0) as total_reserved,
    a.balance - COALESCE(SUM(br.reserved_amount - br.consumed_amount), 0) as available_balance
FROM accounts a
LEFT JOIN balance_reservations br ON a.id = br.account_id AND br.status = 'active'
GROUP BY a.id, a.account_code, a.account_name, a.balance;

-- Vista de resumen de CDRs por cuenta
CREATE OR REPLACE VIEW v_cdr_summary AS
SELECT 
    account_id,
    DATE(start_time) as call_date,
    COUNT(*) as total_calls,
    SUM(duration) as total_duration_seconds,
    SUM(billsec) as total_billsec_seconds,
    SUM(cost) as total_cost
FROM call_detail_records
GROUP BY account_id, DATE(start_time);

-- =============================================================================
-- PERMISOS (ajusta según tu usuario)
-- =============================================================================

-- Asumiendo que tu usuario es 'postgres' o el que uses en tu app
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO postgres;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO postgres;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO postgres;

-- =============================================================================
-- VERIFICACIÓN
-- =============================================================================

-- Verificar tablas creadas
SELECT table_name 
FROM information_schema.tables 
WHERE table_schema = 'public' 
ORDER BY table_name;

-- Verificar ENUMs
SELECT typname, enumlabel 
FROM pg_type 
JOIN pg_enum ON pg_type.oid = pg_enum.enumtypid
WHERE typname IN ('account_type', 'account_status', 'reservation_status', 'reservation_type', 'transaction_type')
ORDER BY typname, enumlabel;

-- Verificar datos iniciales
SELECT account_code, account_name, account_type, balance FROM accounts;
SELECT destination_prefix, destination_name, rate_per_minute FROM rate_cards ORDER BY priority DESC;
SELECT * FROM v_available_balance;

VACUUM ANALYZE;
