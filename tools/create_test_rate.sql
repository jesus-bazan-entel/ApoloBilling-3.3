-- ═══════════════════════════════════════════════════════════════════════════
-- CREAR TARIFA DE PRUEBA para destino 51 (Perú)
-- Apolo Billing Engine v2.0.5
-- ═══════════════════════════════════════════════════════════════════════════

\set ON_ERROR_STOP on

\echo ''
\echo '╔═══════════════════════════════════════════════════════════════════════════╗'
\echo '║              CREAR TARIFA DE PRUEBA - Destino 51 (Perú)                 ║'
\echo '╚═══════════════════════════════════════════════════════════════════════════╝'
\echo ''

-- Paso 1: Verificar rate_cards existentes
\echo '📊 Paso 1: Rate cards existentes:'
SELECT COUNT(*) as total_rate_cards FROM rate_cards;

\echo ''
\echo '📋 Rate cards actuales (primeras 5):'
SELECT id, destination_prefix, destination_name, rate_per_minute, billing_increment, connection_fee, effective_start, priority
FROM rate_cards
ORDER BY destination_prefix
LIMIT 5;

\echo ''
\echo '─────────────────────────────────────────────────────────────────────────────'

-- Paso 2: Crear tarifa para Perú (51)
\echo ''
\echo '📝 Paso 2: Creando tarifa para destino 51 (Perú)...'

-- Eliminar tarifa existente si ya existe
DELETE FROM rate_cards WHERE destination_prefix = '51';

-- Insertar nueva tarifa
INSERT INTO rate_cards (
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
    ('51', 'Peru - Mobile/Fixed', 0.018, 6, 0.00, NOW(), NULL, 10)
RETURNING
    id,
    destination_prefix,
    destination_name,
    rate_per_minute,
    billing_increment,
    connection_fee,
    effective_start,
    effective_end,
    priority;

\echo ''
\echo '─────────────────────────────────────────────────────────────────────────────'

-- Paso 3: Verificar todas las rate_cards
\echo ''
\echo '📊 Paso 3: Todas las rate cards (después de insertar):'
SELECT id, destination_prefix, destination_name, rate_per_minute, billing_increment, connection_fee, effective_start, priority
FROM rate_cards
ORDER BY destination_prefix;

\echo ''
\echo '═══════════════════════════════════════════════════════════════════════════'
\echo '✅ TARIFA CREADA'
\echo ''
\echo '📌 Tarifa configurada:'
\echo '   • Prefix: 51 (Perú)'
\echo '   • Rate: $0.018/min'
\echo '   • Billing increment: 6 seconds'
\echo '   • Description: Peru - Mobile/Fixed'
\echo ''
\echo '🚀 Próximos pasos:'
\echo ''
\echo '   1️⃣  Reiniciar motor Rust (no es necesario recompilar):'
\echo '       Ctrl+C en Terminal 1'
\echo '       RUST_LOG=info cargo run'
\echo ''
\echo '   2️⃣  Ejecutar simulador nuevamente:'
\echo '       cd /home/jbazan/ApoloBilling'
\echo '       ./tools/esl_simulator.py --duration 30'
\echo ''
\echo '   3️⃣  Verificar logs - DEBE mostrar:'
\echo '       ✅ "Found account: 100001"'
\echo '       ✅ "Rate found: Peru - $0.018/min"'
\echo '       ✅ "Call authorized"'
\echo '       ✅ "Balance reserved"'
\echo '       ✅ "CDR saved successfully"'
\echo ''
\echo '═══════════════════════════════════════════════════════════════════════════'
\echo ''
