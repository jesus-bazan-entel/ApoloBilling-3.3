-- Crear cuenta de prueba 100001 para simulaciones
-- Ejecutar: sudo -u postgres psql -d apolo_billing -f create_test_account.sql

\echo '╔══════════════════════════════════════════════════════════════════════════════╗'
\echo '║         Crear Cuenta de Prueba - Apolo Billing Engine v2.0.5               ║'
\echo '╚══════════════════════════════════════════════════════════════════════════════╝'
\echo ''

-- Verificar tabla accounts
\echo '🔍 Verificando tabla accounts...'
\d accounts

-- Insertar cuenta de prueba
\echo ''
\echo '📝 Creando cuenta de prueba 100001 con $10.00...'
INSERT INTO accounts (account_number, account_name, balance, account_type, status)
VALUES ('100001', 'Test Account', 10.00, 'PREPAID', 'ACTIVE')
ON CONFLICT (account_number) DO UPDATE 
SET balance = 10.00, 
    account_name = 'Test Account',
    account_type = 'PREPAID',
    status = 'ACTIVE'
RETURNING id, account_number, account_name, balance, account_type, status;

-- Mostrar todas las cuentas
\echo ''
\echo '📊 Todas las cuentas existentes:'
SELECT id, account_number, account_name, balance, account_type, status 
FROM accounts 
ORDER BY id;

\echo ''
\echo '═══════════════════════════════════════════════════════════════════════════════'
\echo '✅ CUENTA DE PRUEBA LISTA'
\echo ''
\echo '   Account Number: 100001'
\echo '   Balance: $10.00'
\echo '   Type: PREPAID'
\echo '   Status: ACTIVE'
\echo ''
\echo '🚀 Ahora puedes ejecutar el simulador:'
\echo '   cd /home/jbazan/ApoloBilling'
\echo '   ./tools/esl_simulator.py --duration 30'
\echo '═══════════════════════════════════════════════════════════════════════════════'
