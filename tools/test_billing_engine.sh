#!/bin/bash

###############################################################################
# Script de Testing del Motor de Billing Rust con Simulador ESL
# Autor: GenSpark AI Developer
# Descripción: Prueba el motor de billing simulando eventos de FreeSWITCH
###############################################################################

set -e

# Colores
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║      Testing del Motor de Billing Rust - Apolo Billing Engine       ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Verificar que estamos en el directorio correcto
if [ ! -d "rust-billing-engine" ]; then
    log_error "Directorio rust-billing-engine no encontrado"
    log_info "Ejecuta este script desde el directorio raíz del proyecto"
    exit 1
fi

# Verificar Python
if ! command -v python3 &> /dev/null; then
    log_error "Python 3 no está instalado"
    exit 1
fi

log_success "Python 3 encontrado: $(python3 --version)"

# Verificar PostgreSQL
log_info "Verificando PostgreSQL..."
if ! sudo service postgresql status > /dev/null 2>&1; then
    log_warning "PostgreSQL no está corriendo, iniciando..."
    sudo service postgresql start
fi
log_success "PostgreSQL está corriendo"

# Verificar Redis
log_info "Verificando Redis..."
if ! sudo service redis-server status > /dev/null 2>&1; then
    log_warning "Redis no está corriendo, iniciando..."
    sudo service redis-server start
fi
log_success "Redis está corriendo"

# Verificar que la base de datos existe
log_info "Verificando base de datos..."
if ! sudo -u postgres psql -lqt | cut -d \| -f 1 | grep -qw apolo_billing; then
    log_error "Base de datos apolo_billing no existe"
    log_info "Ejecuta primero: cd backend && python init_db_clean.py"
    exit 1
fi
log_success "Base de datos apolo_billing existe"

# Verificar que existe una cuenta de prueba
log_info "Verificando cuenta de prueba..."
ACCOUNT_EXISTS=$(sudo -u postgres psql -d apolo_billing -tAc \
    "SELECT COUNT(*) FROM accounts WHERE account_number = '100001';")

if [ "$ACCOUNT_EXISTS" -eq "0" ]; then
    log_warning "Cuenta 100001 no existe, creándola..."
    
    # Verificar si la tabla tiene la columna account_name
    HAS_ACCOUNT_NAME=$(sudo -u postgres psql -d apolo_billing -tAc \
        "SELECT COUNT(*) FROM information_schema.columns 
         WHERE table_name='accounts' AND column_name='account_name';")
    
    if [ "$HAS_ACCOUNT_NAME" -eq "1" ]; then
        # Esquema nuevo con account_name
        sudo -u postgres psql -d apolo_billing << 'EOF'
INSERT INTO accounts (account_number, account_name, balance, account_type, status, max_concurrent_calls)
VALUES ('100001', 'Cuenta Demo Test', 10.00, 'PREPAID', 'ACTIVE', 5)
ON CONFLICT (account_number) DO NOTHING;
EOF
    else
        # Esquema legacy sin account_name
        sudo -u postgres psql -d apolo_billing << 'EOF'
INSERT INTO accounts (account_number, balance, account_type, status, max_concurrent_calls)
VALUES ('100001', 10.00, 'PREPAID', 'ACTIVE', 5)
ON CONFLICT (account_number) DO NOTHING;
EOF
    fi
    log_success "Cuenta 100001 creada con balance $10.00"
else
    # Actualizar balance para pruebas
    sudo -u postgres psql -d apolo_billing << 'EOF'
UPDATE accounts 
SET balance = 10.00
WHERE account_number = '100001';
EOF
    log_success "Cuenta 100001 actualizada (balance: $10.00)"
fi

# Verificar que existen rate cards
log_info "Verificando rate cards..."

# Primero verificar qué columna de prefijo tiene la tabla
HAS_DEST_PREFIX=$(sudo -u postgres psql -d apolo_billing -tAc \
    "SELECT COUNT(*) FROM information_schema.columns 
     WHERE table_name='rate_cards' AND column_name='destination_prefix';")

if [ "$HAS_DEST_PREFIX" -eq "1" ]; then
    PREFIX_COLUMN="destination_prefix"
else
    PREFIX_COLUMN="prefix"
fi

RATECARD_COUNT=$(sudo -u postgres psql -d apolo_billing -tAc \
    "SELECT COUNT(*) FROM rate_cards WHERE $PREFIX_COLUMN = '519';")

if [ "$RATECARD_COUNT" -eq "0" ]; then
    log_warning "Rate card para Perú Móvil (519) no existe, creándola..."
    
    if [ "$HAS_DEST_PREFIX" -eq "1" ]; then
        # Esquema nuevo
        sudo -u postgres psql -d apolo_billing << 'EOF'
INSERT INTO rate_cards (destination_prefix, destination_name, rate_per_minute, billing_increment, priority)
VALUES ('519', 'Perú Móvil', 0.0180, 6, 150)
ON CONFLICT DO NOTHING;
EOF
    else
        # Esquema legacy
        sudo -u postgres psql -d apolo_billing << 'EOF'
INSERT INTO rate_cards (prefix, destination, rate, increment, priority)
VALUES ('519', 'Perú Móvil', 0.0180, 6, 150)
ON CONFLICT DO NOTHING;
EOF
    fi
    log_success "Rate card creada: 519 (Perú Móvil) - $0.0180/min"
else
    log_success "Rate cards existen"
fi

echo ""
log_info "Estado de la cuenta 100001:"

# Verificar columnas disponibles
HAS_ACCOUNT_NAME=$(sudo -u postgres psql -d apolo_billing -tAc \
    "SELECT COUNT(*) FROM information_schema.columns 
     WHERE table_name='accounts' AND column_name='account_name';")

if [ "$HAS_ACCOUNT_NAME" -eq "1" ]; then
    sudo -u postgres psql -d apolo_billing -c \
        "SELECT account_number, account_name, balance, account_type, status FROM accounts WHERE account_number = '100001';"
else
    sudo -u postgres psql -d apolo_billing -c \
        "SELECT account_number, balance, account_type, status FROM accounts WHERE account_number = '100001';"
fi

echo ""

# =====================================================================
# Compilar motor Rust (si es necesario)
# =====================================================================
log_info "Verificando motor de billing Rust..."

if [ ! -f "rust-billing-engine/target/debug/billing-engine" ] && \
   [ ! -f "rust-billing-engine/target/release/billing-engine" ]; then
    log_warning "Motor Rust no compilado, compilando..."
    cd rust-billing-engine
    cargo build
    cd ..
    log_success "Motor Rust compilado"
else
    log_success "Motor Rust ya está compilado"
fi

# =====================================================================
# Preparar simulador
# =====================================================================
log_info "Preparando simulador ESL..."
chmod +x tools/esl_simulator.py

echo ""
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                         INSTRUCCIONES DE USO                         ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""
echo "📋 PASOS PARA PROBAR EL MOTOR DE BILLING:"
echo ""
echo "1️⃣  INICIAR EL MOTOR RUST (Terminal 1):"
echo "    cd rust-billing-engine"
echo "    cargo run"
echo "    "
echo "    O en modo release (más rápido):"
echo "    cargo run --release"
echo ""
echo "2️⃣  EJECUTAR SIMULADOR ESL (Terminal 2 - Esta ventana):"
echo ""
echo "    Prueba rápida (1 llamada de 30 segundos):"
echo "    ./tools/esl_simulator.py --duration 30"
echo ""
echo "    Prueba completa (5 llamadas de 60 segundos):"
echo "    ./tools/esl_simulator.py --duration 60 --calls 5 --delay 10"
echo ""
echo "    Prueba personalizada:"
echo "    ./tools/esl_simulator.py \\"
echo "        --caller 100001 \\"
echo "        --callee 51987654321 \\"
echo "        --duration 120 \\"
echo "        --account 100001 \\"
echo "        --calls 3"
echo ""
echo "3️⃣  MONITOREAR RESULTADOS:"
echo ""
echo "    Logs del motor Rust (Terminal 1):"
echo "    - CHANNEL_CREATE (autorización)"
echo "    - CHANNEL_ANSWER (inicio billing)"
echo "    - CHANNEL_HANGUP (CDR generado)"
echo ""
echo "    Base de datos PostgreSQL:"
echo "    sudo -u postgres psql -d apolo_billing -c \\"
echo "        \"SELECT * FROM cdrs ORDER BY created_at DESC LIMIT 5;\""
echo ""
echo "    Balance de cuenta:"
echo "    sudo -u postgres psql -d apolo_billing -c \\"
echo "        \"SELECT account_number, balance FROM accounts WHERE account_number = '100001';\""
echo ""
echo "    Reservaciones activas (durante llamada):"
echo "    sudo -u postgres psql -d apolo_billing -c \\"
echo "        \"SELECT * FROM balance_reservations WHERE status = 'ACTIVE';\""
echo ""
echo "4️⃣  DASHBOARD WEB:"
echo "    http://localhost:8000/dashboard/cdr"
echo "    (Asegúrate de tener el servidor FastAPI corriendo)"
echo ""
echo "═══════════════════════════════════════════════════════════════════════"
echo ""

log_info "¿Quieres ejecutar una prueba rápida ahora? (s/n)"
read -r respuesta

if [ "$respuesta" = "s" ]; then
    log_info "Verificando si el motor Rust está corriendo..."
    
    # Intentar conectar al puerto ESL
    if timeout 2 bash -c "echo > /dev/tcp/127.0.0.1/8021" 2>/dev/null; then
        log_success "Motor Rust detectado en puerto 8021"
        echo ""
        log_info "Ejecutando prueba: 1 llamada de 30 segundos..."
        echo ""
        python3 tools/esl_simulator.py --duration 30
    else
        log_error "Motor Rust NO está corriendo en puerto 8021"
        log_info "Por favor, inicia el motor primero en otra terminal:"
        log_info "    cd rust-billing-engine && cargo run"
        exit 1
    fi
else
    log_info "Prueba cancelada. Sigue las instrucciones arriba cuando estés listo."
fi

echo ""
log_success "¡Script completado!"
