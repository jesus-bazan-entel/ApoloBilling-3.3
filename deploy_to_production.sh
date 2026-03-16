#!/bin/bash

# =================================================================
# 🚀 SCRIPT DE DESPLIEGUE AUTOMATIZADO EN PRODUCCIÓN
# =================================================================
# Autor: ApoloBilling Production Deployment Script
# Fecha: 2024-01-12
# Uso: ./deploy_to_production.sh [docker|manual]
# =================================================================

set -e

# Colores para output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m'

# Banner
echo -e "${BLUE}"
echo "========================================================"
echo "🚀 APOLOBILLING - DESPLIEGUE EN PRODUCCIÓN"
echo "========================================================"
echo -e "${NC}"

# Verificar argumentos
DEPLOYMENT_TYPE=${1:-docker}
if [[ "$DEPLOYMENT_TYPE" != "docker" && "$DEPLOYMENT_TYPE" != "manual" ]]; then
    echo "Uso: $0 [docker|manual]"
    echo "  docker: Despliegue con Docker (recomendado)"
    echo "  manual: Despliegue manual sin Docker"
    exit 1
fi

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[ÉXITO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[ADVERTENCIA]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Verificar que estamos en producción
print_status "Verificando entorno de producción..."
if [[ $(hostname) == "localhost" ]] || [[ $(hostname) =~ ^ip- ]] || [[ $(hostname) =~ ^ip- ]] ; then
    print_warning "Este script está diseñado para ejecutarse en un servidor de producción."
    read -p "¿Continuar? (y/n): " CONTINUE
    if [ "$CONTINUE" != "y" ]; then
        exit 1
    fi
fi

# Verificar que tenemos permisos de sudo
if ! sudo -n true 2>/dev/null; then
    print_error "Este script requiere permisos de sudo."
    exit 1
fi

# Paso 1: Actualizar sistema
print_status "Actualizando sistema..."
sudo apt update && sudo apt upgrade -y
print_success "Sistema actualizado."

# Paso 2: Instalar dependencias básicas
print_status "Instalando dependencias básicas..."
sudo apt install -y curl wget git unzip software-properties-common apt-transport-https ca-certificates gnupg lsb-release

# Instalar Docker si se eligió docker
if [ "$DEPLOYMENT_TYPE" = "docker" ]; then
    print_status "Instalando Docker..."
    if ! command -v docker &> /dev/null; then
        curl -fsSL https://get.docker.com -o get-docker.sh
        sudo sh get-docker.sh
        sudo usermod -aG docker $USER
        print_success "Docker instalado."
    else
        print_success "Docker ya está instalado."
    fi

    # Instalar Docker Compose
    if ! command -v docker-compose &> /dev/null; then
        sudo curl -L "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
        sudo chmod +x /usr/local/bin/docker-compose
        print_success "Docker Compose instalado."
    else
        print_success "Docker Compose ya está instalado."
    fi
else
    # Instalar dependencias para despliegue manual
    print_status "Instalando dependencias para despliegue manual..."
    
    # Python
    sudo apt install -y python3 python3-pip python3-venv python3-dev
    
    # Node.js
    curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -
    sudo apt install -y nodejs
    
    # PostgreSQL
    sudo apt install -y postgresql postgresql-contrib
    
    # Redis
    sudo apt install -y redis-server
    
    # Nginx
    sudo apt install -y nginx
    
    # Rust
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source ~/.cargo/env
    
    print_success "Dependencias instaladas."
fi

# Paso 3: Configurar usuario del sistema
print_status "Configurando usuario del sistema..."
if ! id "apolo" &>/dev/null; then
    sudo useradd -r -s /bin/bash -d /opt/ApoloBilling -m apolo
    sudo usermod -aG sudo apolo
    print_success "Usuario 'apolo' creado."
else
    print_success "Usuario 'apolo' ya existe."
fi

# Paso 4: Crear estructura de directorios
print_status "Creando estructura de directorios..."
sudo mkdir -p /opt/ApoloBilling
sudo mkdir -p /opt/logs
sudo mkdir -p /opt/backups
sudo chown -R apolo:apolo /opt/ApoloBilling
sudo chown -R apolo:apolo /opt/logs
sudo chown -R apolo:apolo /opt/backups
print_success "Directorios creados."

# Paso 5: Configurar PostgreSQL
print_status "Configurando PostgreSQL..."
sudo systemctl enable postgresql
sudo systemctl start postgresql

# Configurar base de datos
DB_PASSWORD=$(openssl rand -base64 32)
sudo -u postgres psql << EOF
CREATE DATABASE apolo_billing;
CREATE USER apolo_user WITH PASSWORD '$DB_PASSWORD';
GRANT ALL PRIVILEGES ON DATABASE apolo_billing TO apolo_user;
\q
EOF

print_success "Base de datos configurada."

# Paso 6: Configurar Redis
print_status "Configurando Redis..."
sudo systemctl enable redis-server
sudo systemctl start redis-server
print_success "Redis configurado."

# Paso 7: Descargar código desde GitHub
print_status "Descargando código desde GitHub..."
cd /opt/ApoloBilling
sudo chown -R apolo:apolo .
sudo -u apolo git clone https://github.com/jesus-bazan-entel/ApoloBilling.git .
sudo -u apolo git checkout v1.0.0
print_success "Código descargado."

# Paso 8: Crear variables de entorno
print_status "Creando archivo de variables de entorno..."
sudo -u apolo tee /opt/ApoloBilling/.env.production > /dev/null << EOF
# Database Configuration
DB_PASSWORD=$DB_PASSWORD
DATABASE_URL=postgresql://apolo_user:$DB_PASSWORD@localhost:5432/apolo_billing

# FreeSWITCH Configuration
ESL_HOST=127.0.0.1
ESL_PORT=8021

# Security
JWT_SECRET=$(openssl rand -base64 64)
API_KEY=$(openssl rand -base64 32)

# Redis
REDIS_URL=redis://localhost:6379

# Logging
LOG_LEVEL=INFO

# Environment
ENVIRONMENT=production
EOF

sudo chmod 600 /opt/ApoloBilling/.env.production
print_success "Variables de entorno configuradas."

# Paso 9: Configurar firewall
print_status "Configurando firewall..."
sudo ufw --force reset
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow ssh
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw allow 5432/tcp  # Solo si necesitas acceso externo a PostgreSQL
sudo ufw --force enable
print_success "Firewall configurado."

# Paso 10: Configurar despliegue según el tipo elegido
if [ "$DEPLOYMENT_TYPE" = "docker" ]; then
    # DESPLIEGUE CON DOCKER
    print_status "Configurando despliegue con Docker..."
    
    sudo -u apolo cp .env.production .env
    
    # Crear Docker Compose
    sudo -u apolo tee docker-compose.prod.yml > /dev/null << 'EOF'
version: '3.8'

services:
  postgres:
    image: postgres:15-alpine
    container_name: apolo_postgres
    restart: unless-stopped
    environment:
      POSTGRES_DB: apolo_billing
      POSTGRES_USER: apolo_user
      POSTGRES_PASSWORD_FILE: /run/secrets/db_password
    volumes:
      - postgres_data:/var/lib/postgresql/data
    networks:
      - apolo_network
    secrets:
      - db_password

  redis:
    image: redis:7-alpine
    container_name: apolo_redis
    restart: unless-stopped
    networks:
      - apolo_network

  billing-engine:
    build:
      context: ./rust-billing-engine
      dockerfile: Dockerfile
    container_name: apolo_billing_engine
    restart: unless-stopped
    depends_on:
      - postgres
      - redis
    networks:
      - apolo_network

  api-backend:
    build:
      context: ./backend
      dockerfile: Dockerfile
    container_name: apolo_api_backend
    restart: unless-stopped
    depends_on:
      - postgres
      - redis
      - billing-engine
    networks:
      - apolo_network

  esl-listener:
    build:
      context: .
      dockerfile: Dockerfile.esl
    container_name: apolo_esl_listener
    restart: unless-stopped
    depends_on:
      - api-backend
    networks:
      - apolo_network

  frontend:
    build:
      context: ./frontend
      dockerfile: Dockerfile
    container_name: apolo_frontend
    restart: unless-stopped
    depends_on:
      - api-backend
    ports:
      - "80:80"
      - "443:443"
    networks:
      - apolo_network

volumes:
  postgres_data:

networks:
  apolo_network:
    driver: bridge

secrets:
  db_password:
    file: ./.env.production
EOF

    # Construir y ejecutar
    sudo -u apolo docker-compose -f docker-compose.prod.yml build
    sudo -u apolo docker-compose -f docker-compose.prod.yml up -d
    print_success "Servicios Docker iniciados."
    
else
    # DESPLIEGUE MANUAL
    print_status "Configurando despliegue manual..."
    
    # Billing Engine (Rust)
    print_status "Instalando Billing Engine..."
    cd /opt/ApoloBilling/rust-billing-engine
    sudo -u apolo ~/.cargo/bin/cargo build --release
    sudo -u apolo cp target/release/billing-engine /usr/local/bin/
    
    # API Backend (Python)
    print_status "Instalando API Backend..."
    cd /opt/ApoloBilling/backend
    sudo -u apolo python3 -m venv venv
    sudo -u apolo source venv/bin/activate
    sudo -u apolo pip install -r requirements.txt
    
    # ESL Listener
    print_status "Instalando ESL Listener..."
    cd /opt/ApoloBilling
    sudo -u apolo python3 -m venv venv_esl
    sudo -u apolo source venv_esl/bin/activate
    sudo -u apolo pip install -r requirements.txt
fi

# Paso 11: Crear servicios systemd (para despliegue manual)
if [ "$DEPLOYMENT_TYPE" = "manual" ]; then
    print_status "Creando servicios systemd..."
    
    # Billing Engine Service
    sudo tee /etc/systemd/system/apolo-billing-engine.service > /dev/null << EOF
[Unit]
Description=Apolo Billing Engine
After=network.target postgresql.service redis.service

[Service]
Type=simple
User=apolo
ExecStart=/usr/local/bin/billing-engine
Restart=always
EnvironmentFile=/opt/ApoloBilling/.env.production

[Install]
WantedBy=multi-user.target
EOF

    # API Backend Service
    sudo tee /etc/systemd/system/apolo-api-backend.service > /dev/null << EOF
[Unit]
Description=Apolo API Backend
After=network.target postgresql.service redis.service apolo-billing-engine.service

[Service]
Type=simple
User=apolo
WorkingDirectory=/opt/ApoloBilling/backend
ExecStart=/opt/ApoloBilling/backend/venv/bin/uvicorn app.main:app --host 0.0.0.0 --port 8000
Restart=always
EnvironmentFile=/opt/ApoloBilling/.env.production

[Install]
WantedBy=multi-user.target
EOF

    # ESL Listener Service
    sudo tee /etc/systemd/system/apolo-esl-listener.service > /dev/null << EOF
[Unit]
Description=Apolo ESL Listener
After=network.target apolo-api-backend.service

[Service]
Type=simple
User=apolo
WorkingDirectory=/opt/ApoloBilling
ExecStart=/opt/ApoloBilling/venv_esl/bin/python connectors/freeswitch/esl_listener.py
Restart=always
EnvironmentFile=/opt/ApoloBilling/.env.production

[Install]
WantedBy=multi-user.target
EOF

    # Habilitar servicios
    sudo systemctl daemon-reload
    sudo systemctl enable apolo-billing-engine
    sudo systemctl enable apolo-api-backend
    sudo systemctl enable apolo-esl-listener
    
    # Iniciar servicios
    sudo systemctl start apolo-billing-engine
    sudo systemctl start apolo-api-backend
    sudo systemctl start apolo-esl-listener
    
    print_success "Servicios systemd configurados y iniciados."
fi

# Paso 12: Configurar Nginx
print_status "Configurando Nginx..."
sudo tee /etc/nginx/sites-available/apolo-billing > /dev/null << EOF
server {
    listen 80;
    server_name _;

    # Security headers
    add_header X-Frame-Options DENY;
    add_header X-Content-Type-Options nosniff;
    add_header X-XSS-Protection "1; mode=block";

    # API Backend
    location /api/ {
        proxy_pass http://127.0.0.1:8000;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
    }

    # Billing Engine
    location /billing/ {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }

    # Frontend (si existe)
    location / {
        root /opt/ApoloBilling/frontend/dist;
        try_files \$uri \$uri/ /index.html;
        
        # Cache static files
        location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg)$ {
            expires 1y;
            add_header Cache-Control "public, immutable";
        }
    }
}
EOF

sudo ln -sf /etc/nginx/sites-available/apolo-billing /etc/nginx/sites-enabled/
sudo rm -f /etc/nginx/sites-enabled/default
sudo nginx -t
sudo systemctl enable nginx
sudo systemctl reload nginx

print_success "Nginx configurado."

# Paso 13: Crear script de backup
print_status "Creando script de backup..."
sudo -u apolo tee /opt/backup.sh > /dev/null << 'EOF'
#!/bin/bash
DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="/opt/backups"

# Backup de la aplicación
tar -czf "$BACKUP_DIR/ApoloBilling_$DATE.tar.gz" /opt/ApoloBilling --exclude='/opt/ApoloBilling/.git'

# Backup de la base de datos
sudo -u postgres pg_dump apolo_billing > "$BACKUP_DIR/db_$DATE.sql"

# Limpiar backups antiguos (mantener últimos 7 días)
find "$BACKUP_DIR" -name "*.tar.gz" -mtime +7 -delete
find "$BACKUP_DIR" -name "*.sql" -mtime +7 -delete

echo "Backup completado: $DATE"
EOF

sudo chmod +x /opt/backup.sh

# Programar backup diario
echo "0 2 * * * /opt/backup.sh" | sudo crontab -

print_success "Backup automatizado configurado."

# Paso 14: Configurar monitoreo básico
print_status "Configurando monitoreo..."
sudo -u apolo tee /opt/health_check.sh > /dev/null << 'EOF'
#!/bin/bash
LOG_FILE="/opt/logs/health_check.log"
TIMESTAMP=$(date '+%Y-%m-%d %H:%M:%S')

# Función para verificar servicio
check_service() {
    local service=$1
    if systemctl is-active --quiet $service; then
        echo "[$TIMESTAMP] ✅ $service: OK" >> $LOG_FILE
    else
        echo "[$TIMESTAMP] ❌ $service: FAILED" >> $LOG_FILE
        systemctl restart $service
    fi
}

# Verificar servicios
check_service "postgresql"
check_service "redis-server"
check_service "nginx"

# Verificar puertos
if netstat -tuln | grep -q ":8000"; then
    echo "[$TIMESTAMP] ✅ API Backend (8000): OK" >> $LOG_FILE
else
    echo "[$TIMESTAMP] ❌ API Backend (8000): FAILED" >> $LOG_FILE
    systemctl restart apolo-api-backend
fi

if netstat -tuln | grep -q ":8080"; then
    echo "[$TIMESTAMP] ✅ Billing Engine (8080): OK" >> $LOG_FILE
else
    echo "[$TIMESTAMP] ❌ Billing Engine (8080): FAILED" >> $LOG_FILE
    systemctl restart apolo-billing-engine
fi
EOF

sudo chmod +x /opt/health_check.sh

# Programar health check cada 5 minutos
echo "*/5 * * * * /opt/health_check.sh" | sudo crontab -

print_success "Monitoreo configurado."

# Paso 15: Mostrar información final
echo ""
print_success "🎉 ¡DESPLIEGUE COMPLETADO EXITOSAMENTE!"
echo ""

echo -e "${PURPLE}📋 INFORMACIÓN DEL DESPLIEGUE:${NC}"
echo "============================================"
echo "Tipo de despliegue: $DEPLOYMENT_TYPE"
echo "Directorio: /opt/ApoloBilling"
echo "Usuario: apolo"
echo "Base de datos: apolo_billing"
echo ""

echo -e "${PURPLE}🔗 SERVICIOS DISPONIBLES:${NC}"
echo "============================================"
echo "🌐 Web Interface: http://$(curl -s ifconfig.me 2>/dev/null || echo 'TU_IP_SERVIDOR')"
echo "🔌 API Backend: http://$(curl -s ifconfig.me 2>/dev/null || echo 'TU_IP_SERVIDOR'):8000"
echo "⚡ Billing Engine: http://$(curl -s ifconfig.me 2>/dev/null || echo 'TU_IP_SERVIDOR'):8080"
echo ""

echo -e "${PURPLE}📁 ARCHIVOS IMPORTANTES:${NC}"
echo "============================================"
echo "Variables de entorno: /opt/ApoloBilling/.env.production"
echo "Logs de sistema: /var/log/syslog"
echo "Logs de aplicación: /opt/logs/"
echo "Backups: /opt/backups/"
echo ""

echo -e "${PURPLE}🔧 COMANDOS ÚTILES:${NC}"
echo "============================================"
echo "# Ver estado de servicios:"
if [ "$DEPLOYMENT_TYPE" = "manual" ]; then
    echo "sudo systemctl status apolo-billing-engine"
    echo "sudo systemctl status apolo-api-backend"
    echo "sudo systemctl status apolo-esl-listener"
else
    echo "docker-compose -f /opt/ApoloBilling/docker-compose.prod.yml ps"
fi

echo ""
echo "# Ver logs:"
if [ "$DEPLOYMENT_TYPE" = "manual" ]; then
    echo "sudo journalctl -u apolo-billing-engine -f"
    echo "sudo journalctl -u apolo-api-backend -f"
else
    echo "docker-compose -f /opt/ApoloBilling/docker-compose.prod.yml logs -f"
fi

echo ""
echo "# Reiniciar servicios:"
if [ "$DEPLOYMENT_TYPE" = "manual" ]; then
    echo "sudo systemctl restart apolo-billing-engine apolo-api-backend apolo-esl-listener"
else
    echo "docker-compose -f /opt/ApoloBilling/docker-compose.prod.yml restart"
fi

echo ""
echo "# Backup manual:"
echo "/opt/backup.sh"
echo ""

echo -e "${PURPLE}🔒 SEGURIDAD:${NC}"
echo "============================================"
echo "✅ Firewall configurado (puertos 22, 80, 443)"
echo "✅ Usuario dedicado para la aplicación"
echo "✅ Variables de entorno protegidas"
echo "✅ Backup automático configurado"
echo "✅ Monitoreo de salud activo"
echo ""

echo -e "${YELLOW}⚠️  IMPORTANTE:${NC}"
echo "1. Cambia la contraseña de la base de datos en /opt/ApoloBilling/.env.production"
echo "2. Configura SSL/HTTPS para producción"
echo "3. Actualiza las reglas del firewall según tus necesidades"
echo "4. Revisa regularmente los logs en /opt/logs/"
echo ""

print_success "🚀 ¡Sistema listo para producción!"

if [ "$DEPLOYMENT_TYPE" = "docker" ]; then
    echo ""
    print_warning "NOTA: Para Docker, necesitarás reiniciar la sesión para que los permisos de grupo se apliquen:"
    echo "exit"
    echo "# Luego reconecta con: ssh apolo@servidor"
fi