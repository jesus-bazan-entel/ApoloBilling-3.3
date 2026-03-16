-- Setup test schema for Rust Billing Engine Integration Tests
-- Run with: sudo -u postgres psql -d apolo_billing -f setup_test_schema.sql

-- Drop existing tables if they exist (for clean test runs)
DROP TABLE IF EXISTS balance_transactions CASCADE;
DROP TABLE IF EXISTS call_detail_records CASCADE;
DROP TABLE IF EXISTS balance_reservations CASCADE;
DROP TABLE IF EXISTS accounts CASCADE;

-- Create accounts table
CREATE TABLE accounts (
    id SERIAL PRIMARY KEY,
    account_number VARCHAR(50) UNIQUE NOT NULL,
    account_name VARCHAR(100),
    account_type VARCHAR(20) NOT NULL DEFAULT 'prepaid',
    balance DECIMAL(10,4) NOT NULL DEFAULT 0,
    credit_limit DECIMAL(10,4) NOT NULL DEFAULT 0,
    currency VARCHAR(3) NOT NULL DEFAULT 'USD',
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    max_concurrent_calls INTEGER DEFAULT 5,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_accounts_number ON accounts(account_number);
CREATE INDEX idx_accounts_status ON accounts(status);

-- Create balance_reservations table
CREATE TABLE balance_reservations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id INTEGER NOT NULL REFERENCES accounts(id),
    call_uuid VARCHAR(100) NOT NULL,
    reserved_amount DECIMAL(10,4) NOT NULL,
    consumed_amount DECIMAL(10,4) NOT NULL DEFAULT 0,
    released_amount DECIMAL(10,4) NOT NULL DEFAULT 0,
    status VARCHAR(30) NOT NULL DEFAULT 'active',
    reservation_type VARCHAR(20) NOT NULL DEFAULT 'initial',
    destination_prefix VARCHAR(20),
    rate_per_minute DECIMAL(10,6) NOT NULL,
    reserved_minutes INTEGER NOT NULL DEFAULT 5,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    consumed_at TIMESTAMP WITH TIME ZONE,
    released_at TIMESTAMP WITH TIME ZONE,
    created_by VARCHAR(100)
);

CREATE INDEX idx_reservations_account ON balance_reservations(account_id);
CREATE INDEX idx_reservations_call ON balance_reservations(call_uuid);
CREATE INDEX idx_reservations_status ON balance_reservations(status);
CREATE INDEX idx_reservations_expires ON balance_reservations(expires_at);

-- Create balance_transactions table (audit log)
CREATE TABLE balance_transactions (
    id BIGSERIAL PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts(id),
    amount DECIMAL(10,4) NOT NULL,
    previous_balance DECIMAL(10,4) NOT NULL,
    new_balance DECIMAL(10,4) NOT NULL,
    transaction_type VARCHAR(50) NOT NULL,
    reason TEXT,
    call_uuid VARCHAR(100),
    reservation_id UUID REFERENCES balance_reservations(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    created_by VARCHAR(100)
);

CREATE INDEX idx_transactions_account ON balance_transactions(account_id);
CREATE INDEX idx_transactions_call ON balance_transactions(call_uuid);
CREATE INDEX idx_transactions_type ON balance_transactions(transaction_type);
CREATE INDEX idx_transactions_created ON balance_transactions(created_at);

-- Create call_detail_records table
CREATE TABLE call_detail_records (
    id BIGSERIAL PRIMARY KEY,
    call_uuid VARCHAR(100) UNIQUE NOT NULL,
    account_id INTEGER REFERENCES accounts(id),
    caller_number VARCHAR(50) NOT NULL,
    callee_number VARCHAR(50) NOT NULL,
    destination_prefix VARCHAR(20),
    start_time TIMESTAMP WITH TIME ZONE,
    answer_time TIMESTAMP WITH TIME ZONE,
    end_time TIMESTAMP WITH TIME ZONE,
    duration INTEGER DEFAULT 0,
    billsec INTEGER DEFAULT 0,
    rate_card_id INTEGER,
    rate_per_minute DECIMAL(10,6),
    cost DECIMAL(10,4) DEFAULT 0,
    hangup_cause VARCHAR(50),
    hangup_disposition VARCHAR(20),
    reservation_id UUID REFERENCES balance_reservations(id),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMP WITH TIME ZONE
);

CREATE INDEX idx_cdr_call ON call_detail_records(call_uuid);
CREATE INDEX idx_cdr_account ON call_detail_records(account_id);
CREATE INDEX idx_cdr_start ON call_detail_records(start_time);
CREATE INDEX idx_cdr_caller ON call_detail_records(caller_number);
CREATE INDEX idx_cdr_callee ON call_detail_records(callee_number);

-- Insert test accounts
INSERT INTO accounts (account_number, account_name, account_type, balance, credit_limit, status, max_concurrent_calls) VALUES
    ('100001', 'Test Prepaid Account 1', 'prepaid', 100.00, 0, 'active', 5),
    ('100002', 'Test Prepaid Account 2', 'prepaid', 50.00, 0, 'active', 5),
    ('100003', 'Test Prepaid Low Balance', 'prepaid', 1.00, 0, 'active', 5),
    ('100004', 'Test Prepaid Zero Balance', 'prepaid', 0.00, 0, 'active', 5),
    ('100005', 'Test Suspended Account', 'prepaid', 100.00, 0, 'suspended', 5),
    ('200001', 'Test Postpaid Account 1', 'postpaid', 0.00, 500.00, 'active', 10),
    ('200002', 'Test Postpaid Near Limit', 'postpaid', -450.00, 500.00, 'active', 10);

-- Insert test rate cards (make sure table exists and add test data)
DELETE FROM rate_cards WHERE destination_prefix LIKE '51%' OR destination_prefix LIKE '1%' OR destination_prefix LIKE '44%';

INSERT INTO rate_cards (destination_prefix, destination_name, rate_per_minute, billing_increment, connection_fee, effective_start, priority) VALUES
    -- Peru rates
    ('51', 'Peru - General', 0.015, 6, 0, NOW() - INTERVAL '1 day', 10),
    ('519', 'Peru - Mobile', 0.025, 6, 0, NOW() - INTERVAL '1 day', 50),
    ('5198', 'Peru - Mobile Claro', 0.020, 6, 0, NOW() - INTERVAL '1 day', 60),
    ('5199', 'Peru - Mobile Movistar', 0.022, 6, 0, NOW() - INTERVAL '1 day', 60),
    ('511', 'Peru - Lima', 0.012, 6, 0, NOW() - INTERVAL '1 day', 50),

    -- USA rates
    ('1', 'USA/Canada - General', 0.010, 6, 0, NOW() - INTERVAL '1 day', 10),
    ('1212', 'USA - New York', 0.008, 6, 0, NOW() - INTERVAL '1 day', 50),
    ('1310', 'USA - Los Angeles', 0.008, 6, 0, NOW() - INTERVAL '1 day', 50),
    ('1800', 'USA - Toll Free', 0.000, 60, 0, NOW() - INTERVAL '1 day', 100),

    -- UK rates
    ('44', 'UK - General', 0.020, 6, 0, NOW() - INTERVAL '1 day', 10),
    ('4420', 'UK - London', 0.015, 6, 0, NOW() - INTERVAL '1 day', 50),
    ('447', 'UK - Mobile', 0.050, 6, 0, NOW() - INTERVAL '1 day', 50),

    -- Premium rate for testing
    ('1900', 'USA - Premium', 2.000, 60, 1.00, NOW() - INTERVAL '1 day', 100);

-- Verify setup
SELECT 'Accounts created:' as info, count(*) as count FROM accounts;
SELECT 'Rate cards created:' as info, count(*) as count FROM rate_cards;

-- Show test accounts
SELECT id, account_number, account_type, balance, status FROM accounts ORDER BY id;

-- Show sample rates
SELECT destination_prefix, destination_name, rate_per_minute, billing_increment, priority
FROM rate_cards
WHERE destination_prefix LIKE '51%' OR destination_prefix LIKE '1%'
ORDER BY destination_prefix;
