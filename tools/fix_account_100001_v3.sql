-- ═══════════════════════════════════════════════════════════════════════════
-- CORRECCIÓN FINAL v3: Crear cuenta 100001 (esquema completo)
-- Apolo Billing Engine v2.0.5
-- ═══════════════════════════════════════════════════════════════════════════

\set ON_ERROR_STOP on

\echo ''
\echo '╔═══════════════════════════════════════════════════════════════════════════╗'
\echo '║        CORRECCIÓN FINAL v3: Cuenta de Prueba 100001                     ║'
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

-- Paso 2: Crear o actualizar cuenta con TODOS los campos NOT NULL
\echo '📝 Paso 2: Creando/Actualizando cuenta 100001...'
INSERT INTO accounts (
    account_number, 
    customer_phone,
    account_type, 
    balance,
    credit_limit,
    currency,
    status,
    max_concurrent_calls,
    created_at,
    updated_at
) 
VALUES (
    '100001',                    -- account_number
    '100001',                    -- customer_phone (mismo que account_number)
    'PREPAID',                   -- account_type (ENUM)
    10.00,                       -- balance inicial $10.00
    0.00,                        -- credit_limit (0 para prepago)
    'USD',                       -- currency
    'ACTIVE',                    -- status (ENUM)
    5,                           -- max_concurrent_calls
    NOW(),                       -- created_at
    NOW()                        -- updated_at
)
ON CONFLICT (account_number) 
DO UPDATE SET 
    balance = 10.00,
    credit_limit = 0.00,
    currency = 'USD',
    account_type = 'PREPAID',
    status = 'ACTIVE',
    max_concurrent_calls = 5,
    updated_at = NOW()
RETURNING 
    id, 
    account_number,
    customer_phone,
    account_type, 
    balance,
    credit_limit,
    currency,
    status,
    max_concurrent_calls;

\echo ''
\echo '─────────────────────────────────────────────────────────────────────────────'

-- Paso 3: Verificar resultado
\echo ''
\echo '📊 Paso 3: Verificación final - Todas las cuentas:'
SELECT 
    id,
    account_number,
    customer_phone,
    account_type::text as account_type,
    balance,
    credit_limit,
    currency,
    status::text as status,
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
\echo '📋 Primeras 3 tarifas (si existen):'
SELECT 
    id,
    destination_prefix,
    rate_per_minute,
    billing_increment
FROM rate_cards
ORDER BY destination_prefix
LIMIT 3;

\echo ''
\echo '═══════════════════════════════════════════════════════════════════════════'
\echo '✅ CORRECCIÓN COMPLETADA'
\echo ''
\echo '📌 Cuenta creada/actualizada:'
\echo '   • Account Number: 100001'
\echo '   • Customer Phone: 100001'
\echo '   • Balance: $10.00'
\echo '   • Credit Limit: $0.00 (prepago)'
\echo '   • Currency: USD'
\echo '   • Type: PREPAID'
\echo '   • Status: ACTIVE'
\echo '   • Max Calls: 5'
\echo ''
\echo '🚀 Próximos pasos:'
\echo ''
\echo '   1️⃣  Terminal 1 - Iniciar motor Rust:'
\echo '       cd /home/jbazan/ApoloBilling/rust-billing-engine'
\echo '       RUST_LOG=info cargo run'
\echo ''
\echo '   2️⃣  Terminal 2 - Ejecutar simulador:'
\echo '       cd /home/jbazan/ApoloBilling'
\echo '       ./tools/esl_simulator.py --duration 30'
\echo ''
\echo '   3️⃣  Verificar logs Rust - DEBE mostrar:'
\echo '       ✅ "✅ Call authorized" (caller: 100001, account_id: ...)'
\echo '       ✅ "💰 Balance reserved"'
\echo '       ✅ "📞 Billing started"'
\echo '       ✅ "💵 Billing tick"'
\echo '       ✅ "📊 CDR saved successfully"'
\echo ''
\echo '   4️⃣  Verificar CDR generado:'
\echo '       sudo -u postgres psql -d apolo_billing -c "SELECT call_uuid, caller_number, called_number, duration, cost FROM cdrs ORDER BY created_at DESC LIMIT 1;"'
\echo ''
\echo '   5️⃣  Verificar balance actualizado:'
\echo '       sudo -u postgres psql -d apolo_billing -c "SELECT account_number, balance FROM accounts WHERE account_number = '\''100001'\'';"'
\echo ''
\echo '═══════════════════════════════════════════════════════════════════════════'
\echo ''
