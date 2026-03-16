-- ═══════════════════════════════════════════════════════════════════════════
-- CORRECCIÓN RÁPIDA: Crear cuenta 100001 para testing
-- Apolo Billing Engine v2.0.5
-- ═══════════════════════════════════════════════════════════════════════════

-- Ejecutar este comando:
-- sudo -u postgres psql -d apolo_billing -f tools/fix_account_100001.sql

\set ON_ERROR_STOP on

\echo ''
\echo '╔═══════════════════════════════════════════════════════════════════════════╗'
\echo '║        CORRECCIÓN RÁPIDA: Cuenta de Prueba 100001                       ║'
\echo '╚═══════════════════════════════════════════════════════════════════════════╝'
\echo ''

-- Paso 1: Verificar si la cuenta existe
\echo '🔍 Paso 1: Verificando cuenta 100001...'
SELECT 
    CASE 
        WHEN COUNT(*) > 0 THEN '⚠️  La cuenta 100001 YA EXISTE'
        ELSE '✅ Cuenta 100001 no existe - Procederá a crear'
    END AS status
FROM accounts 
WHERE account_number = '100001';

\echo ''

-- Paso 2: Crear o actualizar cuenta
\echo '📝 Paso 2: Creando/Actualizando cuenta 100001...'
INSERT INTO accounts (
    account_number, 
    account_name, 
    balance, 
    account_type, 
    status,
    max_concurrent_calls,
    created_at,
    updated_at
) 
VALUES (
    '100001',                    -- account_number
    'Test Account',              -- account_name
    10.00,                       -- balance inicial $10.00
    'PREPAID',                   -- account_type
    'ACTIVE',                    -- status
    5,                           -- max_concurrent_calls
    NOW(),                       -- created_at
    NOW()                        -- updated_at
)
ON CONFLICT (account_number) 
DO UPDATE SET 
    balance = 10.00,
    account_name = 'Test Account',
    account_type = 'PREPAID',
    status = 'ACTIVE',
    updated_at = NOW()
RETURNING 
    id, 
    account_number, 
    account_name, 
    balance, 
    account_type, 
    status;

\echo ''
\echo '─────────────────────────────────────────────────────────────────────────────'

-- Paso 3: Verificar resultado
\echo ''
\echo '📊 Paso 3: Verificación final - Todas las cuentas:'
SELECT 
    id,
    account_number,
    account_name,
    balance,
    account_type,
    status,
    max_concurrent_calls
FROM accounts
ORDER BY id;

\echo ''
\echo '─────────────────────────────────────────────────────────────────────────────'

-- Paso 4: Verificar rate_cards
\echo ''
\echo '📊 Paso 4: Verificando rate_cards (tarifas):'
SELECT COUNT(*) as total_rate_cards FROM rate_cards;

\echo ''
\echo '📋 Primeras 3 tarifas:'
SELECT 
    id,
    destination_prefix,
    rate_per_minute,
    billing_increment,
    COALESCE(description, 'Sin descripción') as description
FROM rate_cards
ORDER BY destination_prefix
LIMIT 3;

\echo ''
\echo '═══════════════════════════════════════════════════════════════════════════'
\echo '✅ CORRECCIÓN COMPLETADA'
\echo ''
\echo '📌 Cuenta creada/actualizada:'
\echo '   • Account Number: 100001'
\echo '   • Balance: $10.00'
\echo '   • Type: PREPAID'
\echo '   • Status: ACTIVE'
\echo ''
\echo '🚀 Próximos pasos:'
\echo ''
\echo '   1️⃣  Terminal 1 - Iniciar motor Rust:'
\echo '       cd /home/jbazan/ApoloBilling/rust-billing-engine'
\echo '       RUST_LOG=info cargo run'
\echo ''
\echo '   2️⃣  Terminal 2 - Ejecutar simulador (esperar a que inicie el motor):'
\echo '       cd /home/jbazan/ApoloBilling'
\echo '       ./tools/esl_simulator.py --duration 30'
\echo ''
\echo '   3️⃣  Verificar logs Rust - DEBE mostrar:'
\echo '       ✅ "Call authorized" (caller: 100001)'
\echo '       ✅ "Balance reserved"'
\echo '       ✅ "Billing started"'
\echo '       ✅ "CDR saved successfully"'
\echo ''
\echo '   4️⃣  Verificar CDR generado:'
\echo '       sudo -u postgres psql -d apolo_billing -c "SELECT call_uuid, caller_number, called_number, duration, cost FROM cdrs ORDER BY created_at DESC LIMIT 1;"'
\echo ''
\echo '═══════════════════════════════════════════════════════════════════════════'
\echo ''
