#!/bin/bash
# create_test_account.sh - Crear cuenta de prueba para simulaciones

echo "╔══════════════════════════════════════════════════════════════════════════════╗"
echo "║         Crear Cuenta de Prueba - Apolo Billing Engine v2.0.5               ║"
echo "╚══════════════════════════════════════════════════════════════════════════════╝"
echo ""

# Verificar PostgreSQL
echo "🔍 Verificando PostgreSQL..."
if ! sudo service postgresql status > /dev/null 2>&1; then
    echo "❌ PostgreSQL no está ejecutándose"
    echo "   Iniciando PostgreSQL..."
    sudo service postgresql start
fi

echo "✅ PostgreSQL está activo"
echo ""

# Crear cuenta de prueba
echo "📝 Creando cuenta de prueba 100001 con \$10.00..."
sudo -u postgres psql -d apolo_billing << 'EOF'
-- Insertar cuenta de prueba si no existe
INSERT INTO accounts (account_number, balance, account_type, account_name)
VALUES ('100001', 10.00, 'prepaid', 'Test Account')
ON CONFLICT (account_number) DO UPDATE 
SET balance = 10.00, account_name = 'Test Account'
RETURNING id, account_number, balance, account_type, account_name;

-- Mostrar todas las cuentas
\echo ''
\echo '📊 Cuentas existentes:'
SELECT id, account_number, balance, account_type, account_name 
FROM accounts 
ORDER BY id;
EOF

echo ""
echo "═══════════════════════════════════════════════════════════════════════════════"
echo "✅ CUENTA DE PRUEBA LISTA"
echo ""
echo "   Account Number: 100001"
echo "   Balance: \$10.00"
echo "   Type: prepaid"
echo "   Name: Test Account"
echo ""
echo "🚀 Ahora puedes ejecutar el simulador:"
echo "   ./tools/esl_simulator.py --duration 30"
echo "═══════════════════════════════════════════════════════════════════════════════"
