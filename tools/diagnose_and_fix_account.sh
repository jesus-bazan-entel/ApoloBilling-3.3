#!/bin/bash

################################################################################
# Diagnóstico y Corrección Automática - Cuenta de Prueba 100001
# Apolo Billing Engine v2.0.5
################################################################################

set -e  # Exit on error

echo "╔══════════════════════════════════════════════════════════════════════════════╗"
echo "║      Diagnóstico y Corrección Automática - Cuenta de Prueba 100001         ║"
echo "╚══════════════════════════════════════════════════════════════════════════════╝"
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Database connection
DB_USER="postgres"
DB_NAME="apolo_billing"

echo "════════════════════════════════════════════════════════════════════════════════"
echo "🔍 PASO 1: Verificar conexión a PostgreSQL"
echo "════════════════════════════════════════════════════════════════════════════════"

if sudo -u postgres psql -d "$DB_NAME" -c "\conninfo" > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Conexión exitosa a base de datos: $DB_NAME${NC}"
else
    echo -e "${RED}❌ ERROR: No se puede conectar a la base de datos $DB_NAME${NC}"
    echo ""
    echo "💡 Solución: Verificar que PostgreSQL esté ejecutándose:"
    echo "   sudo systemctl status postgresql"
    echo "   sudo systemctl start postgresql"
    exit 1
fi
echo ""

echo "════════════════════════════════════════════════════════════════════════════════"
echo "🔍 PASO 2: Verificar tabla accounts"
echo "════════════════════════════════════════════════════════════════════════════════"

TABLE_EXISTS=$(sudo -u postgres psql -d "$DB_NAME" -t -c "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'accounts');")

if [[ "$TABLE_EXISTS" =~ "t" ]]; then
    echo -e "${GREEN}✅ Tabla 'accounts' existe${NC}"
    
    # Mostrar esquema
    echo ""
    echo "📋 Esquema de la tabla accounts:"
    sudo -u postgres psql -d "$DB_NAME" -c "\d accounts"
else
    echo -e "${RED}❌ ERROR: Tabla 'accounts' NO existe${NC}"
    echo ""
    echo "💡 Solución: Inicializar la base de datos:"
    echo "   cd /home/jbazan/ApoloBilling/backend"
    echo "   source venv/bin/activate"
    echo "   python init_db_clean.py"
    exit 1
fi
echo ""

echo "════════════════════════════════════════════════════════════════════════════════"
echo "🔍 PASO 3: Verificar cuenta 100001"
echo "════════════════════════════════════════════════════════════════════════════════"

ACCOUNT_EXISTS=$(sudo -u postgres psql -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM accounts WHERE account_number = '100001';")

if [[ "$ACCOUNT_EXISTS" -gt 0 ]]; then
    echo -e "${YELLOW}⚠️  La cuenta 100001 YA EXISTE${NC}"
    echo ""
    echo "📊 Datos actuales:"
    sudo -u postgres psql -d "$DB_NAME" -c "SELECT id, account_number, account_name, balance, account_type, status FROM accounts WHERE account_number = '100001';"
    echo ""
    
    read -p "¿Deseas RESETEAR el balance a $10.00? (s/n): " -n 1 -r
    echo ""
    if [[ $REPLY =~ ^[Ss]$ ]]; then
        echo "🔄 Reseteando cuenta 100001..."
        sudo -u postgres psql -d "$DB_NAME" -c "UPDATE accounts SET balance = 10.00, status = 'ACTIVE' WHERE account_number = '100001' RETURNING id, account_number, balance, status;"
        echo -e "${GREEN}✅ Cuenta reseteada${NC}"
    else
        echo "ℹ️  Cuenta sin cambios"
    fi
else
    echo -e "${YELLOW}⚠️  Cuenta 100001 NO EXISTE - Creando...${NC}"
    echo ""
    
    sudo -u postgres psql -d "$DB_NAME" -c "INSERT INTO accounts (account_number, account_name, balance, account_type, status) VALUES ('100001', 'Test Account', 10.00, 'PREPAID', 'ACTIVE') RETURNING id, account_number, account_name, balance, account_type, status;"
    
    echo -e "${GREEN}✅ Cuenta 100001 creada exitosamente${NC}"
fi
echo ""

echo "════════════════════════════════════════════════════════════════════════════════"
echo "🔍 PASO 4: Verificar rate_cards (tarjetas de tarificación)"
echo "════════════════════════════════════════════════════════════════════════════════"

RATE_COUNT=$(sudo -u postgres psql -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM rate_cards;")

echo "📊 Total de rate_cards: $RATE_COUNT"

if [[ "$RATE_COUNT" -gt 0 ]]; then
    echo -e "${GREEN}✅ Tarjetas de tarificación disponibles${NC}"
    echo ""
    echo "📋 Primeras 3 tarifas:"
    sudo -u postgres psql -d "$DB_NAME" -c "SELECT id, destination_prefix, rate_per_minute, description FROM rate_cards ORDER BY destination_prefix LIMIT 3;"
else
    echo -e "${RED}❌ WARNING: No hay rate_cards configuradas${NC}"
    echo "💡 El motor Rust usará tarifas por defecto"
fi
echo ""

echo "════════════════════════════════════════════════════════════════════════════════"
echo "🔍 PASO 5: Limpiar CDRs antiguos (opcional)"
echo "════════════════════════════════════════════════════════════════════════════════"

CDR_COUNT=$(sudo -u postgres psql -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM cdrs;")
echo "📊 Total de CDRs existentes: $CDR_COUNT"

if [[ "$CDR_COUNT" -gt 0 ]]; then
    read -p "¿Deseas LIMPIAR todos los CDRs antiguos? (s/n): " -n 1 -r
    echo ""
    if [[ $REPLY =~ ^[Ss]$ ]]; then
        sudo -u postgres psql -d "$DB_NAME" -c "TRUNCATE TABLE cdrs RESTART IDENTITY CASCADE;"
        echo -e "${GREEN}✅ CDRs limpiados${NC}"
    else
        echo "ℹ️  CDRs sin cambios"
    fi
fi
echo ""

echo "════════════════════════════════════════════════════════════════════════════════"
echo "📊 RESUMEN FINAL"
echo "════════════════════════════════════════════════════════════════════════════════"

echo ""
echo "🔍 Cuentas actuales:"
sudo -u postgres psql -d "$DB_NAME" -c "SELECT id, account_number, account_name, balance, account_type, status FROM accounts ORDER BY id;"

echo ""
echo "🔍 Últimos 3 CDRs:"
sudo -u postgres psql -d "$DB_NAME" -c "SELECT call_uuid, caller_number, called_number, duration, cost, created_at FROM cdrs ORDER BY created_at DESC LIMIT 3;"

echo ""
echo "════════════════════════════════════════════════════════════════════════════════"
echo -e "${GREEN}✅ DIAGNÓSTICO COMPLETADO${NC}"
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""
echo "🚀 Próximos pasos:"
echo ""
echo "   1️⃣  Terminal 1 - Iniciar motor Rust:"
echo "       cd /home/jbazan/ApoloBilling/rust-billing-engine"
echo "       RUST_LOG=info cargo run"
echo ""
echo "   2️⃣  Terminal 2 - Ejecutar simulador:"
echo "       cd /home/jbazan/ApoloBilling"
echo "       ./tools/esl_simulator.py --duration 30"
echo ""
echo "   3️⃣  Verificar logs del motor Rust - Debe mostrar:"
echo "       ✅ Call authorized (caller: 100001, account_id: 1)"
echo "       ✅ Balance reserved"
echo "       ✅ Billing started"
echo "       ✅ CDR saved successfully"
echo ""
echo "════════════════════════════════════════════════════════════════════════════════"
