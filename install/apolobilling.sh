#!/usr/bin/env bash
#
# ApoloBilling Installer
# =======================
# One-command installation for ApoloBilling VoIP Billing Platform
#
# Usage:
#   curl -sSL https://raw.githubusercontent.com/jesus-bazan-entel/ApoloBillingv3/main/install/apolobilling.sh | bash
#
# Or clone and run:
#   git clone https://github.com/jesus-bazan-entel/ApoloBillingv3.git
#   cd ApoloBillingv3/install
#   ./apolobilling.sh install
#
# Author: ApoloBilling Team
# License: MIT
#

set -e

#==============================================================================
# VARIABLES GLOBALES
#==============================================================================

# Colores
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
NC='\033[0m' # No Color

# Versiones
APOLO_VERSION="2.0.0"
POSTGRES_VERSION="15"
NODE_VERSION="20"
RUST_MIN_VERSION="1.75.0"

# Directorios
APOLO_HOME="/opt/ApoloBilling"
APOLO_INSTALL_DIR="${APOLO_HOME}/install"
APOLO_LOG_DIR="/var/log/apolobilling"
APOLO_BACKUP_DIR="/var/backups/apolobilling"

# GitHub
GITHUB_REPO="jesus-bazan-entel/ApoloBillingv3"
GITHUB_BRANCH="main"

# Configuración generada
DB_PASSWORD=""
JWT_SECRET=""
ADMIN_PASSWORD="admin123"
SERVER_IP=""
DOMAIN=""

#==============================================================================
# FUNCIONES DE UTILIDAD
#==============================================================================

print_banner() {
    clear
    echo -e "${CYAN}"
    cat << 'EOF'

     █████╗ ██████╗  ██████╗ ██╗      ██████╗
    ██╔══██╗██╔══██╗██╔═══██╗██║     ██╔═══██╗
    ███████║██████╔╝██║   ██║██║     ██║   ██║
    ██╔══██║██╔═══╝ ██║   ██║██║     ██║   ██║
    ██║  ██║██║     ╚██████╔╝███████╗╚██████╔╝
    ╚═╝  ╚═╝╚═╝      ╚═════╝ ╚══════╝ ╚═════╝
    ██████╗ ██╗██╗     ██╗     ██╗███╗   ██╗ ██████╗
    ██╔══██╗██║██║     ██║     ██║████╗  ██║██╔════╝
    ██████╔╝██║██║     ██║     ██║██╔██╗ ██║██║  ███╗
    ██╔══██╗██║██║     ██║     ██║██║╚██╗██║██║   ██║
    ██████╔╝██║███████╗███████╗██║██║ ╚████║╚██████╔╝
    ╚═════╝ ╚═╝╚══════╝╚══════╝╚═╝╚═╝  ╚═══╝ ╚═════╝

EOF
    echo -e "${WHITE}         VoIP Billing Platform Installer v${APOLO_VERSION}${NC}"
    echo -e "${BLUE}         https://github.com/${GITHUB_REPO}${NC}"
    echo ""
}

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${WHITE}  $1${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"
}

spinner() {
    local pid=$1
    local delay=0.1
    local spinstr='⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏'
    while [ "$(ps a | awk '{print $1}' | grep $pid)" ]; do
        local temp=${spinstr#?}
        printf " ${CYAN}%c${NC}  " "$spinstr"
        local spinstr=$temp${spinstr%"$temp"}
        sleep $delay
        printf "\b\b\b\b\b"
    done
    printf "    \b\b\b\b"
}

run_cmd() {
    local cmd="$1"
    local msg="$2"

    echo -ne "${BLUE}[....] ${NC}${msg}"

    if eval "$cmd" >> "${APOLO_LOG_DIR}/install.log" 2>&1; then
        echo -e "\r${GREEN}[ OK ] ${NC}${msg}"
        return 0
    else
        echo -e "\r${RED}[FAIL] ${NC}${msg}"
        echo -e "${RED}       Check ${APOLO_LOG_DIR}/install.log for details${NC}"
        return 1
    fi
}

generate_password() {
    local length=${1:-32}
    openssl rand -hex $((length/2))
}

get_server_ip() {
    # Intentar obtener IP pública
    local public_ip=$(curl -s --connect-timeout 5 https://api.ipify.org 2>/dev/null || \
                      curl -s --connect-timeout 5 https://ifconfig.me 2>/dev/null || \
                      curl -s --connect-timeout 5 https://icanhazip.com 2>/dev/null)

    if [[ -n "$public_ip" ]]; then
        echo "$public_ip"
        return
    fi

    # Fallback a IP local
    ip route get 1 2>/dev/null | awk '{print $7; exit}' || hostname -I | awk '{print $1}'
}

check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "Este script debe ejecutarse como root"
        echo -e "Ejecuta: ${CYAN}sudo $0${NC}"
        exit 1
    fi
}

check_os() {
    if [[ ! -f /etc/debian_version ]]; then
        log_error "Este script está diseñado para Debian/Ubuntu"
        exit 1
    fi

    local version=$(cat /etc/debian_version)
    log_info "Sistema operativo: Debian $version"
}

check_resources() {
    local cpu_cores=$(nproc)
    local total_ram=$(free -g | awk '/^Mem:/{print $2}')
    local disk_space=$(df -BG / | awk 'NR==2 {print $4}' | tr -d 'G')

    echo -e "${WHITE}Recursos del sistema:${NC}"
    echo -e "  CPU Cores:    ${cpu_cores} cores $([ $cpu_cores -ge 4 ] && echo -e "${GREEN}✓${NC}" || echo -e "${YELLOW}⚠ (min: 4)${NC}")"
    echo -e "  RAM:          ${total_ram} GB $([ $total_ram -ge 8 ] && echo -e "${GREEN}✓${NC}" || echo -e "${YELLOW}⚠ (min: 8GB)${NC}")"
    echo -e "  Disco libre:  ${disk_space} GB $([ $disk_space -ge 50 ] && echo -e "${GREEN}✓${NC}" || echo -e "${YELLOW}⚠ (min: 50GB)${NC}")"
    echo ""
}

#==============================================================================
# FUNCIONES DE INSTALACIÓN
#==============================================================================

install_base_packages() {
    log_step "1/12 - Instalando paquetes base"

    run_cmd "apt-get update" "Actualizando repositorios"

    run_cmd "DEBIAN_FRONTEND=noninteractive apt-get install -y \
        build-essential \
        pkg-config \
        libssl-dev \
        curl \
        git \
        wget \
        unzip \
        gnupg2 \
        lsb-release \
        ca-certificates \
        apt-transport-https \
        software-properties-common \
        net-tools \
        dnsutils \
        vim \
        htop \
        jq \
        sudo \
        acl \
        openssl" "Instalando herramientas base"
}

install_postgresql() {
    log_step "2/12 - Instalando PostgreSQL ${POSTGRES_VERSION}"

    # Agregar repositorio oficial
    run_cmd "sh -c 'echo \"deb https://apt.postgresql.org/pub/repos/apt \$(lsb_release -cs)-pgdg main\" > /etc/apt/sources.list.d/pgdg.list'" \
        "Agregando repositorio PostgreSQL"

    run_cmd "wget --quiet -O - https://www.postgresql.org/media/keys/ACCC4CF8.asc | apt-key add -" \
        "Agregando clave GPG"

    run_cmd "apt-get update" "Actualizando repositorios"

    run_cmd "DEBIAN_FRONTEND=noninteractive apt-get install -y postgresql-${POSTGRES_VERSION} postgresql-contrib-${POSTGRES_VERSION}" \
        "Instalando PostgreSQL ${POSTGRES_VERSION}"

    run_cmd "systemctl enable postgresql && systemctl start postgresql" \
        "Habilitando servicio PostgreSQL"

    # Crear usuario y base de datos
    DB_PASSWORD=$(generate_password 32)

    sudo -u postgres psql -c "CREATE USER apolo_user WITH PASSWORD '${DB_PASSWORD}';" >> "${APOLO_LOG_DIR}/install.log" 2>&1 || true
    sudo -u postgres psql -c "CREATE DATABASE apolo_billing OWNER apolo_user;" >> "${APOLO_LOG_DIR}/install.log" 2>&1 || true
    sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE apolo_billing TO apolo_user;" >> "${APOLO_LOG_DIR}/install.log" 2>&1
    sudo -u postgres psql -d apolo_billing -c "GRANT ALL ON SCHEMA public TO apolo_user;" >> "${APOLO_LOG_DIR}/install.log" 2>&1

    log_info "Base de datos apolo_billing creada"
}

install_redis() {
    log_step "3/12 - Instalando Redis"

    run_cmd "DEBIAN_FRONTEND=noninteractive apt-get install -y redis-server" \
        "Instalando Redis"

    # Configurar Redis
    cat >> /etc/redis/redis.conf << 'EOF'
maxmemory 512mb
maxmemory-policy allkeys-lru
EOF

    run_cmd "systemctl enable redis-server && systemctl restart redis-server" \
        "Habilitando servicio Redis"
}

install_mysql() {
    log_step "4/12 - Instalando MariaDB (para Kamailio)"

    run_cmd "DEBIAN_FRONTEND=noninteractive apt-get install -y mariadb-server mariadb-client" \
        "Instalando MariaDB"

    run_cmd "systemctl enable mariadb && systemctl start mariadb" \
        "Habilitando servicio MariaDB"

    # Crear base de datos Kamailio
    MYSQL_PASSWORD=$(generate_password 24)

    mysql -u root << EOF
CREATE DATABASE IF NOT EXISTS kamailio;
CREATE USER IF NOT EXISTS 'kamailio'@'localhost' IDENTIFIED BY '${MYSQL_PASSWORD}';
CREATE USER IF NOT EXISTS 'kamailio'@'%' IDENTIFIED BY '${MYSQL_PASSWORD}';
GRANT ALL PRIVILEGES ON kamailio.* TO 'kamailio'@'localhost';
GRANT ALL PRIVILEGES ON kamailio.* TO 'kamailio'@'%';
FLUSH PRIVILEGES;
EOF

    log_info "Base de datos kamailio creada"
}

install_rust() {
    log_step "5/12 - Instalando Rust"

    if command -v rustc &> /dev/null; then
        local current_version=$(rustc --version | awk '{print $2}')
        log_info "Rust ya instalado: v${current_version}"
    else
        run_cmd "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y" \
            "Instalando Rust via rustup"

        source "$HOME/.cargo/env"
    fi

    # Verificar versión
    export PATH="$HOME/.cargo/bin:$PATH"
    local rust_version=$(rustc --version | awk '{print $2}')
    log_info "Rust instalado: v${rust_version}"
}

install_nodejs() {
    log_step "6/12 - Instalando Node.js ${NODE_VERSION}"

    if command -v node &> /dev/null && command -v npm &> /dev/null; then
        local current_version=$(node --version | tr -d 'v' | cut -d. -f1)
        if [[ $current_version -ge 20 ]]; then
            log_info "Node.js ya instalado: $(node --version) con npm $(npm --version)"
            return
        fi
    fi

    # Eliminar versiones anteriores de Node.js que no incluyen npm
    if command -v node &> /dev/null && ! command -v npm &> /dev/null; then
        run_cmd "apt-get remove -y nodejs 2>/dev/null || true" \
            "Eliminando Node.js anterior (sin npm)"
    fi

    # Eliminar paquete Debian si existe (no incluye npm)
    run_cmd "apt-get remove -y nodejs libnode108 2>/dev/null || true" \
        "Eliminando Node.js de Debian (si existe)"

    run_cmd "curl -fsSL https://deb.nodesource.com/setup_${NODE_VERSION}.x | bash -" \
        "Agregando repositorio Node.js ${NODE_VERSION}"

    # Instalar versión de NodeSource explícitamente (evita que Debian tome prioridad)
    local nodesource_ver=$(apt-cache madison nodejs 2>/dev/null | grep nodesource | head -1 | awk '{print $3}')
    if [[ -n "$nodesource_ver" ]]; then
        run_cmd "DEBIAN_FRONTEND=noninteractive apt-get install -y nodejs=${nodesource_ver}" \
            "Instalando Node.js ${NODE_VERSION} (NodeSource)"
    else
        run_cmd "DEBIAN_FRONTEND=noninteractive apt-get install -y nodejs" \
            "Instalando Node.js ${NODE_VERSION}"
    fi

    # Verificar que npm está disponible
    if ! command -v npm &> /dev/null; then
        log_error "npm no se instaló correctamente. Intentando instalar manualmente..."
        run_cmd "apt-get install -y npm 2>/dev/null || true" \
            "Instalando npm como paquete separado"
    fi

    log_info "Node.js instalado: $(node --version), npm: $(npm --version)"
}

clone_repository() {
    log_step "7/12 - Clonando repositorio ApoloBilling"

    if [[ -d "${APOLO_HOME}/.git" ]]; then
        log_info "Repositorio ya existe, actualizando..."
        run_cmd "cd ${APOLO_HOME} && git pull origin ${GITHUB_BRANCH}" \
            "Actualizando repositorio"
    else
        # Backup si existe directorio
        if [[ -d "${APOLO_HOME}" ]]; then
            run_cmd "mv ${APOLO_HOME} ${APOLO_BACKUP_DIR}/ApoloBilling_$(date +%Y%m%d_%H%M%S)" \
                "Backup de instalación existente"
        fi

        run_cmd "git clone https://github.com/${GITHUB_REPO}.git ${APOLO_HOME}" \
            "Clonando repositorio desde GitHub"
    fi
}

setup_database() {
    log_step "8/12 - Configurando base de datos"

    # Ejecutar schema principal
    if [[ -f "${APOLO_HOME}/database/schema.sql" ]]; then
        run_cmd "sudo -u postgres psql -d apolo_billing -f ${APOLO_HOME}/database/schema.sql" \
            "Ejecutando schema principal"
    fi

    # Ejecutar migraciones
    local migrations_dir="${APOLO_HOME}/database/migrations"
    if [[ -d "$migrations_dir" ]]; then
        for migration in $(ls -1 "$migrations_dir"/*.sql 2>/dev/null | sort); do
            local migration_name=$(basename "$migration")
            run_cmd "sudo -u postgres psql -d apolo_billing -f $migration" \
                "Ejecutando migración: ${migration_name}"
        done
    fi

    # Crear usuario administrador con hash generado dinámicamente
    # Requiere un pequeño binario Rust para generar el hash Argon2 compatible
    local admin_hash=""
    if [[ -f "${APOLO_HOME}/rust-backend/target/release/apolo-billing" ]]; then
        # Generar hash usando un script Rust temporal con las mismas dependencias
        mkdir -p /tmp/apolo-hashgen/src
        cat > /tmp/apolo-hashgen/Cargo.toml << 'HASHEOF'
[package]
name = "hashgen"
version = "0.1.0"
edition = "2021"
[dependencies]
argon2 = "0.5"
rand_core = { version = "0.6", features = ["getrandom"] }
HASHEOF
        cat > /tmp/apolo-hashgen/src/main.rs << 'HASHEOF'
use argon2::{password_hash::{PasswordHasher, SaltString}, Argon2};
use rand_core::OsRng;
fn main() {
    let password = std::env::args().nth(1).unwrap_or_else(|| "admin123".to_string());
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default().hash_password(password.as_bytes(), &salt).unwrap();
    print!("{}", hash.to_string());
}
HASHEOF
        admin_hash=$(cd /tmp/apolo-hashgen && cargo run --release -- "${ADMIN_PASSWORD}" 2>/dev/null)
        rm -rf /tmp/apolo-hashgen
    fi

    # Fallback: hash pre-generado para admin123 (generado con Argon2id defaults)
    if [[ -z "$admin_hash" ]]; then
        admin_hash='$argon2id$v=19$m=19456,t=2,p=1$n6bvDYLutGH+6pXqGV51gQ$kCEfkUljW8R3LPdpHZykl8IXViRrUSvMGqkGNRF2YMs'
    fi

    sudo -u postgres psql -d apolo_billing << EOF
INSERT INTO usuarios (username, password, nombre, apellido, role, activo)
VALUES ('admin', '${admin_hash}', 'Administrador', '', 'superadmin', true)
ON CONFLICT (username) DO NOTHING;
EOF

    log_info "Usuario admin creado (password: ${ADMIN_PASSWORD})"
}

build_backend() {
    log_step "9/12 - Compilando Backend Rust"

    export PATH="$HOME/.cargo/bin:$PATH"

    # Generar JWT Secret
    JWT_SECRET=$(generate_password 64)

    # Crear archivo .env
    cat > "${APOLO_HOME}/rust-backend/.env" << EOF
# Database
DATABASE_URL=postgresql://apolo_user:${DB_PASSWORD}@localhost:5432/apolo_billing
DATABASE_MAX_CONNECTIONS=20

# Server
RUST_SERVER_HOST=0.0.0.0
RUST_SERVER_PORT=8000
RUST_SERVER_WORKERS=4

# CORS
CORS_ORIGINS=http://localhost:3000,http://${SERVER_IP}

# JWT
JWT_SECRET=${JWT_SECRET}
JWT_EXPIRATION_SECS=1800

# Redis
REDIS_URL=redis://127.0.0.1:6379

# Kamailio MySQL
KAMAILIO_DATABASE_URL=mysql://kamailio:${MYSQL_PASSWORD}@localhost:3306/kamailio

# Logging
RUST_LOG=apolo_billing=info,apolo_api=info,actix_web=info
EOF

    run_cmd "cd ${APOLO_HOME}/rust-backend && cargo build --release" \
        "Compilando rust-backend (esto puede tomar varios minutos)"

    log_info "Backend compilado exitosamente"
}

build_billing_engine() {
    log_step "10/12 - Compilando Billing Engine"

    export PATH="$HOME/.cargo/bin:$PATH"

    # Crear archivo .env
    cat > "${APOLO_HOME}/rust-billing-engine/.env" << EOF
# Environment
ENVIRONMENT=production

# Server
HOST=0.0.0.0
PORT=9000

# Database
DATABASE_URL=postgresql://apolo_user:${DB_PASSWORD}@localhost:5432/apolo_billing

# Redis
REDIS_URL=redis://127.0.0.1:6379

# FreeSWITCH ESL (vacío = modo testing)
FREESWITCH_SERVERS=

# Logging
RUST_LOG=info,apolo_billing_engine=debug
EOF

    if [[ -d "${APOLO_HOME}/rust-billing-engine" ]]; then
        run_cmd "cd ${APOLO_HOME}/rust-billing-engine && cargo build --release" \
            "Compilando billing-engine"
        log_info "Billing Engine compilado exitosamente"
    else
        log_warn "Directorio rust-billing-engine no encontrado, saltando..."
    fi
}

build_frontend() {
    log_step "11/12 - Compilando Frontend React"

    # Crear archivo .env
    cat > "${APOLO_HOME}/frontend/.env" << EOF
VITE_API_URL=http://${SERVER_IP}
VITE_WS_URL=ws://${SERVER_IP}/ws
EOF

    run_cmd "cd ${APOLO_HOME}/frontend && npm install" \
        "Instalando dependencias npm"

    run_cmd "cd ${APOLO_HOME}/frontend && npm run build" \
        "Compilando frontend para producción"

    log_info "Frontend compilado exitosamente"
}

setup_services() {
    log_step "12/12 - Configurando servicios systemd"

    # Backend Service
    cat > /etc/systemd/system/apolo-backend.service << EOF
[Unit]
Description=ApoloBilling Backend - REST API
After=network.target postgresql.service redis.service
Wants=postgresql.service redis.service

[Service]
Type=simple
User=root
WorkingDirectory=${APOLO_HOME}/rust-backend
EnvironmentFile=${APOLO_HOME}/rust-backend/.env
ExecStart=${APOLO_HOME}/rust-backend/target/release/apolo-billing
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

    run_cmd "echo 'Service file created'" "Creando apolo-backend.service"

    # Billing Engine Service
    if [[ -f "${APOLO_HOME}/rust-billing-engine/target/release/apolo-billing-engine" ]]; then
        cat > /etc/systemd/system/apolo-billing-engine.service << EOF
[Unit]
Description=ApoloBilling Engine - Real-time billing
After=network.target postgresql.service redis.service
Wants=postgresql.service redis.service

[Service]
Type=simple
User=root
WorkingDirectory=${APOLO_HOME}/rust-billing-engine
EnvironmentFile=${APOLO_HOME}/rust-billing-engine/.env
ExecStart=${APOLO_HOME}/rust-billing-engine/target/release/apolo-billing-engine
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF
        run_cmd "echo 'Service file created'" "Creando apolo-billing-engine.service"
    fi

    # Nginx configuration
    run_cmd "DEBIAN_FRONTEND=noninteractive apt-get install -y nginx" \
        "Instalando Nginx"

    cat > /etc/nginx/sites-available/apolobilling.conf << EOF
server {
    listen 80;
    listen [::]:80;
    server_name _;

    root ${APOLO_HOME}/frontend/dist;
    index index.html;

    # Gzip
    gzip on;
    gzip_vary on;
    gzip_min_length 1024;
    gzip_types text/plain text/css application/json application/javascript text/xml application/xml;

    # Security headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;

    # Cache static assets
    location /assets/ {
        expires 1y;
        add_header Cache-Control "public, immutable";
    }

    # API proxy
    location /api/ {
        proxy_pass http://127.0.0.1:8000;
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
    }

    # WebSocket
    location /ws {
        proxy_pass http://127.0.0.1:8000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host \$host;
        proxy_read_timeout 86400s;
    }

    # SPA fallback
    location / {
        try_files \$uri \$uri/ /index.html;
    }

    access_log /var/log/nginx/apolobilling.access.log;
    error_log /var/log/nginx/apolobilling.error.log;
}
EOF

    # Extensión .conf requerida por configuraciones nginx que usan include *.conf
    run_cmd "ln -sf /etc/nginx/sites-available/apolobilling.conf /etc/nginx/sites-enabled/apolobilling.conf && rm -f /etc/nginx/sites-enabled/default" \
        "Configurando Nginx"

    # Habilitar e iniciar servicios
    run_cmd "systemctl daemon-reload" "Recargando systemd"
    run_cmd "systemctl enable apolo-backend" "Habilitando apolo-backend"
    run_cmd "systemctl start apolo-backend" "Iniciando apolo-backend"

    if [[ -f /etc/systemd/system/apolo-billing-engine.service ]]; then
        run_cmd "systemctl enable apolo-billing-engine" "Habilitando apolo-billing-engine"
        run_cmd "systemctl start apolo-billing-engine" "Iniciando apolo-billing-engine"
    fi

    run_cmd "systemctl enable nginx && systemctl restart nginx" "Reiniciando Nginx"
}

#==============================================================================
# FUNCIÓN DE VERIFICACIÓN
#==============================================================================

verify_installation() {
    echo -e "\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${WHITE}                    VERIFICACIÓN DE INSTALACIÓN${NC}"
    echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"

    local all_ok=true

    # PostgreSQL
    if pg_isready -q; then
        echo -e "  ${GREEN}✓${NC} PostgreSQL"
    else
        echo -e "  ${RED}✗${NC} PostgreSQL"
        all_ok=false
    fi

    # Redis
    if redis-cli ping 2>/dev/null | grep -q PONG; then
        echo -e "  ${GREEN}✓${NC} Redis"
    else
        echo -e "  ${RED}✗${NC} Redis"
        all_ok=false
    fi

    # MariaDB
    if mysqladmin ping 2>/dev/null | grep -q alive; then
        echo -e "  ${GREEN}✓${NC} MariaDB"
    else
        echo -e "  ${RED}✗${NC} MariaDB"
        all_ok=false
    fi

    # Backend API
    sleep 2
    if curl -s http://localhost:8000/api/v1/health 2>/dev/null | grep -q -i "ok\|healthy"; then
        echo -e "  ${GREEN}✓${NC} Backend API"
    else
        echo -e "  ${RED}✗${NC} Backend API"
        all_ok=false
    fi

    # Nginx
    if curl -sI http://localhost 2>/dev/null | grep -q "200\|301\|302"; then
        echo -e "  ${GREEN}✓${NC} Nginx"
    else
        echo -e "  ${RED}✗${NC} Nginx"
        all_ok=false
    fi

    echo ""

    if $all_ok; then
        return 0
    else
        return 1
    fi
}

#==============================================================================
# MOSTRAR RESUMEN FINAL
#==============================================================================

show_summary() {
    echo -e "\n${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${WHITE}                    ¡INSTALACIÓN COMPLETADA!${NC}"
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"

    echo -e "${WHITE}Acceso al sistema:${NC}"
    echo -e "  URL:      ${CYAN}http://${SERVER_IP}${NC}"
    echo -e "  Usuario:  ${CYAN}admin${NC}"
    echo -e "  Password: ${CYAN}${ADMIN_PASSWORD}${NC}"
    echo ""

    echo -e "${WHITE}Credenciales de base de datos:${NC}"
    echo -e "  PostgreSQL: ${CYAN}apolo_user / ${DB_PASSWORD}${NC}"
    echo -e "  MySQL:      ${CYAN}kamailio / ${MYSQL_PASSWORD}${NC}"
    echo ""

    echo -e "${WHITE}Comandos útiles:${NC}"
    echo -e "  Ver estado:    ${CYAN}systemctl status apolo-backend${NC}"
    echo -e "  Ver logs:      ${CYAN}journalctl -u apolo-backend -f${NC}"
    echo -e "  Reiniciar:     ${CYAN}systemctl restart apolo-backend${NC}"
    echo ""

    echo -e "${WHITE}Archivos de configuración guardados en:${NC}"
    echo -e "  ${CYAN}${APOLO_HOME}/rust-backend/.env${NC}"
    echo -e "  ${CYAN}${APOLO_HOME}/frontend/.env${NC}"
    echo ""

    # Guardar credenciales en archivo seguro
    local creds_file="${APOLO_HOME}/credentials.txt"
    cat > "$creds_file" << EOF
===============================================
CREDENCIALES APOLOBILLING - $(date)
===============================================

Web UI:
  URL: http://${SERVER_IP}
  Usuario: admin
  Password: ${ADMIN_PASSWORD}

PostgreSQL:
  Host: localhost:5432
  Database: apolo_billing
  User: apolo_user
  Password: ${DB_PASSWORD}

MySQL (Kamailio):
  Host: localhost:3306
  Database: kamailio
  User: kamailio
  Password: ${MYSQL_PASSWORD}

JWT Secret:
  ${JWT_SECRET}

===============================================
¡IMPORTANTE! Elimina este archivo después de
guardar las credenciales en un lugar seguro.
===============================================
EOF

    chmod 600 "$creds_file"

    echo -e "${YELLOW}⚠  Las credenciales se guardaron en:${NC}"
    echo -e "   ${CYAN}${creds_file}${NC}"
    echo -e "${YELLOW}   ¡Guárdalas en un lugar seguro y elimina este archivo!${NC}"
    echo ""
}

#==============================================================================
# FUNCIONES DE COMANDOS
#==============================================================================

cmd_install() {
    print_banner
    check_root
    check_os

    # Detectar IP del servidor
    SERVER_IP=$(get_server_ip)
    log_info "IP del servidor detectada: ${SERVER_IP}"
    echo ""

    check_resources

    # Crear directorios necesarios
    mkdir -p "${APOLO_LOG_DIR}"
    mkdir -p "${APOLO_BACKUP_DIR}"

    # Timestamp de inicio
    local start_time=$(date +%s)

    echo -e "${WHITE}Iniciando instalación de ApoloBilling...${NC}\n"
    echo -e "Log: ${CYAN}${APOLO_LOG_DIR}/install.log${NC}\n"

    # Ejecutar instalación
    install_base_packages
    install_postgresql
    install_redis
    install_mysql
    install_rust
    install_nodejs
    clone_repository
    setup_database
    build_backend
    build_billing_engine
    build_frontend
    setup_services

    # Verificar
    if verify_installation; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        local minutes=$((duration / 60))
        local seconds=$((duration % 60))

        show_summary

        echo -e "${GREEN}Instalación completada en ${minutes}m ${seconds}s${NC}\n"
    else
        log_error "Algunos servicios no están funcionando correctamente"
        echo -e "Revisa los logs: ${CYAN}journalctl -u apolo-backend -n 50${NC}"
        exit 1
    fi
}

cmd_uninstall() {
    print_banner
    check_root

    echo -e "${YELLOW}⚠  Esto eliminará ApoloBilling y todos sus datos.${NC}"
    read -p "¿Estás seguro? (escribe 'SI' para confirmar): " confirm

    if [[ "$confirm" != "SI" ]]; then
        echo "Cancelado."
        exit 0
    fi

    log_step "Desinstalando ApoloBilling..."

    # Detener servicios
    systemctl stop apolo-backend apolo-billing-engine 2>/dev/null || true
    systemctl disable apolo-backend apolo-billing-engine 2>/dev/null || true

    # Eliminar servicios
    rm -f /etc/systemd/system/apolo-backend.service
    rm -f /etc/systemd/system/apolo-billing-engine.service
    rm -f /etc/nginx/sites-enabled/apolobilling /etc/nginx/sites-enabled/apolobilling.conf
    rm -f /etc/nginx/sites-available/apolobilling /etc/nginx/sites-available/apolobilling.conf

    systemctl daemon-reload
    systemctl restart nginx 2>/dev/null || true

    # Eliminar base de datos (opcional)
    read -p "¿Eliminar base de datos PostgreSQL? (y/N): " del_db
    if [[ "$del_db" =~ ^[Yy]$ ]]; then
        sudo -u postgres psql -c "DROP DATABASE IF EXISTS apolo_billing;" 2>/dev/null || true
        sudo -u postgres psql -c "DROP USER IF EXISTS apolo_user;" 2>/dev/null || true
    fi

    # Eliminar directorio
    read -p "¿Eliminar directorio ${APOLO_HOME}? (y/N): " del_dir
    if [[ "$del_dir" =~ ^[Yy]$ ]]; then
        rm -rf "${APOLO_HOME}"
    fi

    log_info "ApoloBilling desinstalado"
}

cmd_status() {
    print_banner

    echo -e "${WHITE}Estado de los servicios:${NC}\n"

    for service in apolo-backend apolo-billing-engine postgresql redis-server mariadb nginx; do
        local status=$(systemctl is-active $service 2>/dev/null || echo "not-found")
        case $status in
            active)
                echo -e "  ${GREEN}●${NC} $service: ${GREEN}activo${NC}"
                ;;
            inactive)
                echo -e "  ${RED}●${NC} $service: ${RED}inactivo${NC}"
                ;;
            *)
                echo -e "  ${YELLOW}○${NC} $service: ${YELLOW}no instalado${NC}"
                ;;
        esac
    done

    echo ""
    verify_installation
}

cmd_help() {
    print_banner

    echo -e "${WHITE}Uso:${NC} $0 <comando>\n"
    echo -e "${WHITE}Comandos disponibles:${NC}"
    echo -e "  ${CYAN}install${NC}     Instala ApoloBilling completo"
    echo -e "  ${CYAN}uninstall${NC}   Desinstala ApoloBilling"
    echo -e "  ${CYAN}status${NC}      Muestra el estado de los servicios"
    echo -e "  ${CYAN}upgrade${NC}     Actualiza a la última versión"
    echo -e "  ${CYAN}help${NC}        Muestra esta ayuda"
    echo ""

    echo -e "${WHITE}Ejemplos:${NC}"
    echo -e "  ${CYAN}$0 install${NC}"
    echo -e "  ${CYAN}$0 status${NC}"
    echo ""

    echo -e "${WHITE}Instalación rápida (una línea):${NC}"
    echo -e "  ${CYAN}curl -sSL https://raw.githubusercontent.com/${GITHUB_REPO}/${GITHUB_BRANCH}/install/apolobilling.sh | sudo bash -s install${NC}"
    echo ""
}

cmd_upgrade() {
    print_banner
    check_root

    log_step "Actualizando ApoloBilling..."

    # Backup
    local backup_name="backup_$(date +%Y%m%d_%H%M%S)"
    run_cmd "cp -r ${APOLO_HOME}/rust-backend/.env ${APOLO_BACKUP_DIR}/${backup_name}_backend.env" \
        "Backup de configuración backend"
    run_cmd "cp -r ${APOLO_HOME}/frontend/.env ${APOLO_BACKUP_DIR}/${backup_name}_frontend.env 2>/dev/null || true" \
        "Backup de configuración frontend"

    # Pull updates
    run_cmd "cd ${APOLO_HOME} && git pull origin ${GITHUB_BRANCH}" \
        "Descargando actualizaciones"

    # Recompilar
    export PATH="$HOME/.cargo/bin:$PATH"

    run_cmd "cd ${APOLO_HOME}/rust-backend && cargo build --release" \
        "Recompilando backend"

    if [[ -d "${APOLO_HOME}/rust-billing-engine" ]]; then
        run_cmd "cd ${APOLO_HOME}/rust-billing-engine && cargo build --release" \
            "Recompilando billing engine"
    fi

    run_cmd "cd ${APOLO_HOME}/frontend && npm install && npm run build" \
        "Recompilando frontend"

    # Ejecutar migraciones nuevas
    local migrations_dir="${APOLO_HOME}/database/migrations"
    if [[ -d "$migrations_dir" ]]; then
        for migration in $(ls -1 "$migrations_dir"/*.sql 2>/dev/null | sort); do
            local migration_name=$(basename "$migration")
            run_cmd "sudo -u postgres psql -d apolo_billing -f $migration 2>/dev/null || true" \
                "Ejecutando migración: ${migration_name}"
        done
    fi

    # Reiniciar servicios
    run_cmd "systemctl restart apolo-backend" "Reiniciando backend"

    if systemctl is-active apolo-billing-engine &>/dev/null; then
        run_cmd "systemctl restart apolo-billing-engine" "Reiniciando billing engine"
    fi

    run_cmd "systemctl restart nginx" "Reiniciando nginx"

    verify_installation

    log_info "Actualización completada"
}

#==============================================================================
# MAIN
#==============================================================================

main() {
    local command="${1:-help}"

    case "$command" in
        install)
            cmd_install
            ;;
        uninstall)
            cmd_uninstall
            ;;
        status)
            cmd_status
            ;;
        upgrade)
            cmd_upgrade
            ;;
        help|--help|-h)
            cmd_help
            ;;
        *)
            log_error "Comando desconocido: $command"
            cmd_help
            exit 1
            ;;
    esac
}

# Ejecutar
main "$@"
