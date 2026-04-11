#!/bin/bash
#
# ApoloBilling Update Script
# Actualiza el sistema desde GitHub, ejecuta migraciones y recompila
#
# Uso: bash /opt/ApoloBilling/update_apolo.sh
#

set -e

# Colores para output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Directorio base
APOLO_DIR="/opt/ApoloBilling"
BACKUP_DIR="/opt/backups"
DATE=$(date +%Y%m%d_%H%M%S)

echo -e "${GREEN}=========================================="
echo "  ACTUALIZANDO APOLOBILLING"
echo -e "==========================================${NC}"

# Crear directorio de backups si no existe
mkdir -p $BACKUP_DIR

# 1. Backup de base de datos
echo -e "\n${YELLOW}[1/7] Creando backup de base de datos...${NC}"
if command -v pg_dump &> /dev/null; then
    PGPASSWORD="${PGPASSWORD:-}" pg_dump -U apolo_user -h localhost -d apolo_billing > "$BACKUP_DIR/apolo_backup_$DATE.sql" 2>/dev/null || \
    sudo -u postgres pg_dump apolo_billing > "$BACKUP_DIR/apolo_backup_$DATE.sql"
    echo -e "${GREEN}✓ Backup guardado en $BACKUP_DIR/apolo_backup_$DATE.sql${NC}"
else
    echo -e "${RED}⚠ pg_dump no encontrado, saltando backup${NC}"
fi

# 2. Backup de configuración
echo -e "\n${YELLOW}[2/7] Respaldando configuración...${NC}"
if [ -f "$APOLO_DIR/rust-backend/.env" ]; then
    cp "$APOLO_DIR/rust-backend/.env" "$BACKUP_DIR/backend_env_$DATE"
    echo -e "${GREEN}✓ .env respaldado${NC}"
fi

# 3. Actualizar código desde GitHub
echo -e "\n${YELLOW}[3/7] Actualizando código desde GitHub...${NC}"
cd $APOLO_DIR

# Guardar cambios locales
git stash 2>/dev/null || true

# Obtener últimos cambios
git fetch origin main
git pull origin main

echo -e "${GREEN}✓ Código actualizado${NC}"
git log --oneline -3

# 4. Ejecutar migraciones de base de datos
echo -e "\n${YELLOW}[4/7] Ejecutando migraciones de base de datos...${NC}"
if [ -d "$APOLO_DIR/database/migrations" ]; then
    for f in $APOLO_DIR/database/migrations/*.sql; do
        if [ -f "$f" ]; then
            echo "  Ejecutando: $(basename $f)"
            sudo -u postgres psql -d apolo_billing -f "$f" 2>&1 | grep -v "already exists" || true
        fi
    done
    echo -e "${GREEN}✓ Migraciones completadas${NC}"
else
    echo -e "${RED}⚠ Directorio de migraciones no encontrado${NC}"
fi

# 5. Compilar Backend Rust
echo -e "\n${YELLOW}[5/7] Compilando backend Rust...${NC}"
cd $APOLO_DIR/rust-backend

if command -v cargo &> /dev/null; then
    cargo build --release
    echo -e "${GREEN}✓ Backend compilado${NC}"
else
    echo -e "${RED}✗ Cargo no encontrado. Instalar Rust primero.${NC}"
    exit 1
fi

# 6. Compilar Frontend
echo -e "\n${YELLOW}[6/7] Compilando frontend...${NC}"
cd $APOLO_DIR/frontend

if command -v npm &> /dev/null; then
    npm install --silent
    npm run build
    echo -e "${GREEN}✓ Frontend compilado${NC}"
else
    echo -e "${RED}✗ npm no encontrado. Instalar Node.js primero.${NC}"
    exit 1
fi

# 7. Reiniciar servicios
echo -e "\n${YELLOW}[7/7] Reiniciando servicios...${NC}"

if systemctl is-active --quiet apolo-backend; then
    systemctl restart apolo-backend
    echo -e "${GREEN}✓ apolo-backend reiniciado${NC}"
else
    echo -e "${YELLOW}⚠ apolo-backend no está como servicio${NC}"
fi

if systemctl is-active --quiet apolo-billing-engine 2>/dev/null; then
    systemctl restart apolo-billing-engine
    echo -e "${GREEN}✓ apolo-billing-engine reiniciado${NC}"
fi

# Esperar a que el backend inicie
sleep 3

# Verificación final
echo -e "\n${GREEN}=========================================="
echo "  VERIFICACIÓN"
echo -e "==========================================${NC}"

# Health check
echo -e "\n${YELLOW}Health check:${NC}"
if curl -s http://localhost:8000/api/v1/health | grep -q "ok"; then
    echo -e "${GREEN}✓ Backend API respondiendo correctamente${NC}"
else
    echo -e "${RED}✗ Backend API no responde${NC}"
    echo "  Ver logs: journalctl -u apolo-backend -n 50"
fi

# Mostrar versión/commit actual
echo -e "\n${YELLOW}Versión actual:${NC}"
cd $APOLO_DIR
git log --oneline -1

# Mostrar tablas de routing
echo -e "\n${YELLOW}Tablas de routing:${NC}"
sudo -u postgres psql -d apolo_billing -c "SELECT 'routing_trunks' as tabla, count(*) FROM routing_trunks UNION ALL SELECT 'sip_devices', count(*) FROM sip_devices;" 2>/dev/null || true

echo -e "\n${GREEN}=========================================="
echo "  ACTUALIZACIÓN COMPLETADA"
echo -e "==========================================${NC}"
echo ""
echo "Acceder al sistema: http://$(hostname -I | awk '{print $1}')"
echo "Ver logs: journalctl -u apolo-backend -f"
echo ""
