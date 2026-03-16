-- Script de migración completo
\c apolo_billing

BEGIN;

-- 1. Verificar y renombrar columnas en accounts
DO $$ 
BEGIN
    -- Renombrar account_code a account_number si existe
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'accounts' AND column_name = 'account_code'
    ) THEN
        ALTER TABLE accounts RENAME COLUMN account_code TO account_number;
        RAISE NOTICE 'Renamed account_code to account_number';
    END IF;
    
    -- Añadir max_concurrent_calls si no existe
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'accounts' AND column_name = 'max_concurrent_calls'
    ) THEN
        ALTER TABLE accounts ADD COLUMN max_concurrent_calls INTEGER DEFAULT 5;
        RAISE NOTICE 'Added max_concurrent_calls column';
    END IF;
END $$;

-- 2. Actualizar índices y constraints de accounts
DO $$
BEGIN
    -- Drop old constraint if exists
    IF EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'accounts_account_code_key'
    ) THEN
        ALTER TABLE accounts DROP CONSTRAINT accounts_account_code_key;
        RAISE NOTICE 'Dropped accounts_account_code_key constraint';
    END IF;
    
    -- Add new constraint if not exists
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'accounts_account_number_key'
    ) THEN
        ALTER TABLE accounts ADD CONSTRAINT accounts_account_number_key UNIQUE (account_number);
        RAISE NOTICE 'Added accounts_account_number_key constraint';
    END IF;
END $$;

-- Drop old index and create new one
DROP INDEX IF EXISTS idx_accounts_code;
CREATE INDEX IF NOT EXISTS idx_accounts_number ON accounts(account_number);

-- 3. Actualizar rate_cards
DO $$ 
BEGIN
    -- Renombrar rate_increment_seconds a billing_increment
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'rate_cards' AND column_name = 'rate_increment_seconds'
    ) THEN
        ALTER TABLE rate_cards RENAME COLUMN rate_increment_seconds TO billing_increment;
        RAISE NOTICE 'Renamed rate_increment_seconds to billing_increment';
    END IF;
    
    -- Añadir connection_fee si no existe
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'rate_cards' AND column_name = 'connection_fee'
    ) THEN
        ALTER TABLE rate_cards ADD COLUMN connection_fee DECIMAL(10, 6) DEFAULT 0.0;
        RAISE NOTICE 'Added connection_fee column';
    END IF;
    
    -- Renombrar effective_date a effective_start
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'rate_cards' AND column_name = 'effective_date'
    ) THEN
        ALTER TABLE rate_cards RENAME COLUMN effective_date TO effective_start;
        RAISE NOTICE 'Renamed effective_date to effective_start';
    END IF;
    
    -- Renombrar expiry_date a effective_end
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'rate_cards' AND column_name = 'expiry_date'
    ) THEN
        ALTER TABLE rate_cards RENAME COLUMN expiry_date TO effective_end;
        RAISE NOTICE 'Renamed expiry_date to effective_end';
    END IF;
END $$;

-- 4. Cambiar tipos de fecha a TIMESTAMPTZ
DO $$
BEGIN
    -- effective_start
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'rate_cards' 
        AND column_name = 'effective_start' 
        AND data_type != 'timestamp with time zone'
    ) THEN
        ALTER TABLE rate_cards 
        ALTER COLUMN effective_start TYPE TIMESTAMPTZ 
        USING effective_start::TIMESTAMPTZ;
        RAISE NOTICE 'Changed effective_start to TIMESTAMPTZ';
    END IF;
    
    -- effective_end
    IF EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'rate_cards' 
        AND column_name = 'effective_end'
        AND data_type != 'timestamp with time zone'
    ) THEN
        ALTER TABLE rate_cards 
        ALTER COLUMN effective_end TYPE TIMESTAMPTZ 
        USING effective_end::TIMESTAMPTZ;
        RAISE NOTICE 'Changed effective_end to TIMESTAMPTZ';
    END IF;
END $$;

COMMIT;

-- Verificación final
SELECT '=== ACCOUNTS STRUCTURE ===' as info;
SELECT column_name, data_type, is_nullable, column_default
FROM information_schema.columns 
WHERE table_name = 'accounts'
ORDER BY ordinal_position;

SELECT '=== ACCOUNTS DATA ===' as info;
SELECT id, account_number, account_type, status, balance, max_concurrent_calls FROM accounts;

SELECT '=== RATE_CARDS STRUCTURE ===' as info;
SELECT column_name, data_type, is_nullable, column_default
FROM information_schema.columns 
WHERE table_name = 'rate_cards'
ORDER BY ordinal_position;

SELECT '=== RATE_CARDS DATA ===' as info;
SELECT id, destination_prefix, destination_name, rate_per_minute, billing_increment, connection_fee 
FROM rate_cards LIMIT 5;

VACUUM ANALYZE accounts;
VACUUM ANALYZE rate_cards;
