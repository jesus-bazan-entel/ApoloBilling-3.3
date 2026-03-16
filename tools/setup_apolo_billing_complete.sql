-- ═══════════════════════════════════════════════════════════════════════════
-- SCRIPT COMPLETO PARA BASE DE DATOS: apolo_billing
-- Apolo Billing Engine v2.0.5
-- ═══════════════════════════════════════════════════════════════════════════

\set ON_ERROR_STOP on

\echo ''
\echo '╔═══════════════════════════════════════════════════════════════════════════╗'
\echo '║       CONFIGURACIÓN COMPLETA - Base de Datos: apolo_billing              ║'
\echo '╚═══════════════════════════════════════════════════════════════════════════╝'
\echo ''

-- ═══════════════════════════════════════════════════════════════════════════
-- PASO 1: CREAR CUENTA DE PRUEBA 100001
-- ═══════════════════════════════════════════════════════════════════════════

\echo ''
\echo '📊 Paso 1: Verificando cuenta 100001...'
SELECT COUNT(*) as cuenta_existe FROM accounts WHERE account_number = '100001';

\echo ''
\echo '📝 Creando/actualizando cuenta 100001...'

-- Eliminar si existe
DELETE FROM accounts WHERE account_number = '100001';

-- Resetear la secuencia si es necesario
SELECT setval('accounts_id_seq', (SELECT COALESCE(MAX(id), 0) FROM accounts));

-- Crear cuenta nueva (sin especificar id, se auto-genera)
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
    'Test Account 100001',       -- account_name
    10.00,                       -- balance ($10.00)
    'PREPAID',                   -- account_type
    'ACTIVE',                    -- status
    5,                           -- max_concurrent_calls
    NOW(),                       -- created_at
    NOW()                        -- updated_at
)
RETURNING 
    id,
    account_number,
    account_name,
    balance,
    account_type,
    status,
    max_concurrent_calls;

\echo ''
\echo '✅ Cuenta 100001 creada exitosamente'
\echo ''


-- ═══════════════════════════════════════════════════════════════════════════
-- PASO 2: CREAR RATE CARD PARA PERÚ (PREFIX 51)
-- ═══════════════════════════════════════════════════════════════════════════

\echo '─────────────────────────────────────────────────────────────────────────────'
\echo ''
\echo '📊 Paso 2: Verificando rate cards existentes...'
SELECT COUNT(*) as total_rate_cards FROM rate_cards;

\echo ''
\echo '📋 Rate cards actuales (primeras 5):'
SELECT id, rate_name, destination_prefix, destination_name, rate_per_minute, billing_increment, 
       connection_fee, priority
FROM rate_cards
ORDER BY destination_prefix
LIMIT 5;

\echo ''
\echo '📝 Creando tarifa para destino 51 (Perú)...'

-- Eliminar tarifa existente si ya existe
DELETE FROM rate_cards WHERE destination_prefix = '51';

-- Insertar nueva tarifa
INSERT INTO rate_cards (
    rate_name,
    destination_prefix,
    destination_name,
    rate_per_minute,
    billing_increment,
    connection_fee,
    effective_start,
    effective_end,
    priority
)
VALUES
    ('PERU_MOBILE', '51', 'Perú Móvil', 0.018, 6, 0.00, NOW(), NULL, 150)
RETURNING
    id,
    rate_name,
    destination_prefix,
    destination_name,
    rate_per_minute,
    billing_increment,
    connection_fee,
    effective_start,
    effective_end,
    priority;

\echo ''
\echo '✅ Tarifa para Perú (prefix 51) creada exitosamente'
\echo ''


-- ═══════════════════════════════════════════════════════════════════════════
-- PASO 3: VERIFICACIÓN FINAL
-- ═══════════════════════════════════════════════════════════════════════════

\echo '─────────────────────────────────────────────────────────────────────────────'
\echo ''
\echo '📊 Paso 3: Verificación final de configuración'
\echo ''

\echo '✅ CUENTA 100001:'
SELECT 
    id,
    account_number,
    account_name,
    balance,
    account_type,
    status,
    max_concurrent_calls,
    created_at
FROM accounts
WHERE account_number = '100001';

\echo ''
\echo '✅ RATE CARD PERÚ (PREFIX 51):'
SELECT 
    id,
    rate_name,
    destination_prefix,
    destination_name,
    rate_per_minute,
    billing_increment,
    connection_fee,
    effective_start,
    effective_end,
    priority,
    created_at
FROM rate_cards
WHERE destination_prefix = '51';

\echo ''
\echo '📊 Todas las rate cards disponibles:'
SELECT 
    id,
    rate_name,
    destination_prefix, 
    destination_name, 
    rate_per_minute, 
    billing_increment,
    priority
FROM rate_cards
ORDER BY destination_prefix;

\echo ''
\echo '═══════════════════════════════════════════════════════════════════════════'
\echo '✅ CONFIGURACIÓN COMPLETADA EXITOSAMENTE'
\echo '═══════════════════════════════════════════════════════════════════════════'
\echo ''
\echo '📌 Resumen de configuración:'
\echo '   • Base de datos: apolo_billing'
\echo '   • Cuenta: 100001 (PREPAID, Balance: $10.00, Status: ACTIVE)'
\echo '   • Rate: 51 (Perú Móvil, $0.018/min, 6 sec increment)'
\echo ''
\echo '🚀 Próximos pasos:'
\echo ''
\echo '   Terminal 1 - Iniciar motor Rust:'
\echo '   --------------------------------'
\echo '   cd /home/jbazan/ApoloBilling/rust-billing-engine'
\echo '   git pull origin genspark_ai_developer'
\echo '   RUST_LOG=info cargo run'
\echo ''
\echo '   Terminal 2 - Ejecutar simulador:'
\echo '   ---------------------------------'
\echo '   cd /home/jbazan/ApoloBilling'
\echo '   ./tools/esl_simulator.py --duration 30'
\echo ''
\echo '   Verificación - Logs esperados:'
\echo '   ------------------------------'
\echo '   ✅ Found account: 100001 (ID: X, Type: PREPAID, Balance: $10.0000)'
\echo '   ✅ Rate card loaded: Perú Móvil ($0.0180/min, 6 sec increment, priority 150)'
\echo '   ✅ Reservation created: ... Amount: $0.3, Max duration: 1000s'
\echo '   ✅ Call AUTHORIZED'
\echo '   💵 Billing tick: Cost so far: $0.003'
\echo '   💵 Billing tick: Cost so far: $0.006'
\echo '   💵 Billing tick: Cost so far: $0.009'
\echo '   📊 CDR saved successfully (cost: $0.009)'
\echo '   💰 Balance consumed: $0.009 (reserved: $0.3, released: $0.291)'
\echo ''
\echo '═══════════════════════════════════════════════════════════════════════════'
\echo ''
