#!/bin/bash

###############################################################################
# Script para reinicializar base de datos con esquema limpio
# Usar SOLO si hay problemas de esquema incompatible
###############################################################################

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}⚠️  ADVERTENCIA: Este script eliminará TODOS los datos de la base de datos${NC}"
echo -e "${YELLOW}    apolo_billing y la recreará con esquema limpio.${NC}"
echo ""
echo "¿Estás seguro de continuar? (escribe 'SI' para confirmar)"
read -r confirmacion

if [ "$confirmacion" != "SI" ]; then
    echo -e "${RED}❌ Operación cancelada${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}Reinicializando base de datos...${NC}"

# Eliminar y recrear base de datos
sudo -u postgres psql << 'EOF'
DROP DATABASE IF EXISTS apolo_billing;
DROP USER IF EXISTS apolo_user;
CREATE USER apolo_user WITH PASSWORD 'apolo_password_2024';
CREATE DATABASE apolo_billing OWNER apolo_user;
GRANT ALL PRIVILEGES ON DATABASE apolo_billing TO apolo_user;
EOF

echo -e "${GREEN}✅ Base de datos recreada${NC}"
echo ""
echo -e "${GREEN}Inicializando tablas con esquema limpio...${NC}"

# Navegar al directorio backend
cd "$(dirname "$0")/../backend"

# Activar venv si existe
if [ -d "venv" ]; then
    source venv/bin/activate
fi

# Ejecutar init_db_clean.py
if [ -f "init_db_clean.py" ]; then
    python3 init_db_clean.py
    echo ""
    echo -e "${GREEN}✅ Base de datos reinicializada correctamente${NC}"
    echo ""
    echo "Puedes ahora ejecutar:"
    echo "  ./tools/test_billing_engine.sh"
else
    echo -e "${RED}❌ Error: init_db_clean.py no encontrado${NC}"
    exit 1
fi
