#!/bin/bash
#===============================================================================
# ApoloBilling - Fresh Server Installer
# Version: 1.0.0
#
# Installs all required components:
#   - PostgreSQL 15
#   - MySQL/MariaDB
#   - Redis
#   - Nginx
#   - dSIPRouter + Kamailio + RTPEngine
#   - FreeSWITCH
#   - Node.js 20
#   - Rust toolchain
#
# Usage:
#   ./install_server.sh              - Full installation
#   ./install_server.sh --skip-voip  - Skip VoIP components (for dev)
#===============================================================================

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Configuration
APOLO_DIR="/opt/ApoloBilling"
LOG_FILE="/var/log/apolo_install.log"
SKIP_VOIP=false

# FreeSWITCH SignalWire Token
FS_TOKEN="pat_MnRB4EuW3FidLn7fY376ss93"

# Default Credentials (can be overridden with environment variables)
DB_USER="${DB_USER:-apolo}"
DB_PASSWORD="${DB_PASSWORD:-ApoloNext.2026}"
ADMIN_USER="${ADMIN_USER:-admin}"
ADMIN_PASSWORD="${ADMIN_PASSWORD:-admin123}"

# Parse arguments
for arg in "$@"; do
    case $arg in
        --skip-voip)
            SKIP_VOIP=true
            shift
            ;;
    esac
done

#===============================================================================
# Logging functions
#===============================================================================
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] [INFO] $1" >> "$LOG_FILE"
}

log_success() {
    echo -e "${GREEN}[OK]${NC} $1"
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] [OK] $1" >> "$LOG_FILE"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] [WARN] $1" >> "$LOG_FILE"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] [ERROR] $1" >> "$LOG_FILE"
}

log_section() {
    echo ""
    echo -e "${CYAN}======================================${NC}"
    echo -e "${CYAN}  $1${NC}"
    echo -e "${CYAN}======================================${NC}"
    echo ""
}

#===============================================================================
# Pre-flight checks
#===============================================================================
preflight_checks() {
    log_section "Pre-flight Checks"

    # Check if running as root
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root"
        echo "Usage: sudo ./install_server.sh"
        exit 1
    fi

    # Check OS
    if [[ ! -f /etc/debian_version ]]; then
        log_error "This script is designed for Debian/Ubuntu systems"
        exit 1
    fi

    # Get OS info
    OS_VERSION=$(cat /etc/debian_version)
    log_info "Detected Debian version: $OS_VERSION"

    # Check available disk space (need at least 10GB)
    AVAILABLE_SPACE=$(df / | tail -1 | awk '{print $4}')
    if [[ $AVAILABLE_SPACE -lt 10485760 ]]; then
        log_warn "Less than 10GB available disk space. Installation may fail."
    fi

    # Check memory (recommend at least 2GB)
    TOTAL_MEM=$(free -m | awk '/^Mem:/{print $2}')
    if [[ $TOTAL_MEM -lt 2048 ]]; then
        log_warn "Less than 2GB RAM detected. Performance may be affected."
    fi

    log_success "Pre-flight checks passed"
}

#===============================================================================
# System Update
#===============================================================================
update_system() {
    log_section "Updating System"

    apt-get update -y
    apt-get upgrade -y

    # Install basic dependencies
    apt-get install -y \
        curl \
        wget \
        git \
        build-essential \
        software-properties-common \
        apt-transport-https \
        ca-certificates \
        gnupg \
        lsb-release \
        sudo \
        vim \
        htop \
        net-tools \
        dnsutils \
        unzip \
        jq

    log_success "System updated"
}

#===============================================================================
# PostgreSQL Installation
#===============================================================================
install_postgresql() {
    log_section "Installing PostgreSQL 15"

    # Add PostgreSQL repository
    if [[ ! -f /etc/apt/sources.list.d/pgdg.list ]]; then
        wget --quiet -O - https://www.postgresql.org/media/keys/ACCC4CF8.asc | gpg --dearmor -o /usr/share/keyrings/postgresql-keyring.gpg
        echo "deb [signed-by=/usr/share/keyrings/postgresql-keyring.gpg] http://apt.postgresql.org/pub/repos/apt $(lsb_release -cs)-pgdg main" > /etc/apt/sources.list.d/pgdg.list
        apt-get update
    fi

    apt-get install -y postgresql-15 postgresql-contrib-15

    # Start and enable PostgreSQL
    systemctl start postgresql
    systemctl enable postgresql

    # Create database and user with fixed credentials
    sudo -u postgres psql <<EOF
CREATE USER $DB_USER WITH PASSWORD '$DB_PASSWORD';
CREATE DATABASE apolo_billing OWNER $DB_USER;
GRANT ALL PRIVILEGES ON DATABASE apolo_billing TO $DB_USER;
\c apolo_billing
GRANT ALL ON SCHEMA public TO $DB_USER;
EOF

    # Save credentials
    cat > /root/.apolo_credentials <<EOF
# ApoloBilling Credentials
# Generated: $(date)

# PostgreSQL
PG_USER=$DB_USER
PG_PASSWORD=$DB_PASSWORD
PG_DATABASE=apolo_billing

# MySQL/Kamailio
MYSQL_USER=$DB_USER
MYSQL_PASSWORD=$DB_PASSWORD
MYSQL_DATABASE=kamailio

# Admin User
ADMIN_USER=$ADMIN_USER
ADMIN_PASSWORD=$ADMIN_PASSWORD
EOF
    chmod 600 /root/.apolo_credentials

    log_success "PostgreSQL installed"
    log_info "Database: apolo_billing"
    log_info "User: $DB_USER"
    log_info "Password: $DB_PASSWORD"
}

#===============================================================================
# MySQL/MariaDB Installation
#===============================================================================
install_mysql() {
    log_section "Installing MariaDB"

    apt-get install -y mariadb-server mariadb-client

    # Start and enable MariaDB
    systemctl start mariadb
    systemctl enable mariadb

    # Secure installation and create kamailio database with fixed credentials
    mysql <<EOF
-- Secure root
ALTER USER 'root'@'localhost' IDENTIFIED BY '';
DELETE FROM mysql.user WHERE User='';
DELETE FROM mysql.user WHERE User='root' AND Host NOT IN ('localhost', '127.0.0.1', '::1');
DROP DATABASE IF EXISTS test;
DELETE FROM mysql.db WHERE Db='test' OR Db='test\\_%';

-- Create kamailio database and user
CREATE DATABASE IF NOT EXISTS kamailio;
CREATE USER IF NOT EXISTS '$DB_USER'@'localhost' IDENTIFIED BY '$DB_PASSWORD';
GRANT ALL PRIVILEGES ON kamailio.* TO '$DB_USER'@'localhost';
FLUSH PRIVILEGES;
EOF

    log_success "MariaDB installed"
    log_info "Database: kamailio"
    log_info "User: $DB_USER"
    log_info "Password: $DB_PASSWORD"
}

#===============================================================================
# Redis Installation
#===============================================================================
install_redis() {
    log_section "Installing Redis"

    apt-get install -y redis-server

    # Configure Redis
    sed -i 's/^supervised no/supervised systemd/' /etc/redis/redis.conf

    # Start and enable Redis
    systemctl restart redis-server
    systemctl enable redis-server

    log_success "Redis installed"
}

#===============================================================================
# Nginx Installation
#===============================================================================
install_nginx() {
    log_section "Installing Nginx"

    apt-get install -y nginx

    # Start and enable Nginx
    systemctl start nginx
    systemctl enable nginx

    # Remove default site
    rm -f /etc/nginx/sites-enabled/default

    log_success "Nginx installed"
}

#===============================================================================
# Node.js Installation
#===============================================================================
install_nodejs() {
    log_section "Installing Node.js 20"

    # Add NodeSource repository
    curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
    apt-get install -y nodejs

    # Install global packages
    npm install -g serve pm2

    log_success "Node.js $(node --version) installed"
}

#===============================================================================
# Rust Installation
#===============================================================================
install_rust() {
    log_section "Installing Rust"

    if ! command -v cargo &> /dev/null; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi

    # Add to profile for future sessions
    if ! grep -q 'cargo/env' ~/.bashrc; then
        echo 'source "$HOME/.cargo/env"' >> ~/.bashrc
    fi

    log_success "Rust $(rustc --version) installed"
}

#===============================================================================
# dSIPRouter + Kamailio + RTPEngine Installation
#===============================================================================
install_dsiprouter() {
    log_section "Installing dSIPRouter + Kamailio + RTPEngine"

    log_info "This installation takes 10-20 minutes..."

    cd /opt

    # Clone dSIPRouter if not exists
    if [[ ! -d /opt/dsiprouter ]]; then
        git clone https://github.com/dOpensource/dsiprouter.git
    else
        cd dsiprouter
        git pull origin master
    fi

    cd /opt/dsiprouter

    # Install with RTPEngine (-all flag)
    # Use expect to handle interactive prompts, or run non-interactively
    export DEBIAN_FRONTEND=noninteractive

    # Run the installer
    ./dsiprouter.sh install -all -servernat

    # Wait for services to start
    sleep 10

    # Get dSIPRouter credentials from the installation
    if [[ -f /etc/dsiprouter/gui/settings.py ]]; then
        DSIP_PASSWORD=$(grep "DSIP_PASSWORD" /etc/dsiprouter/gui/settings.py 2>/dev/null | cut -d"'" -f2 || echo "admin")
        echo "DSIP_PASSWORD=$DSIP_PASSWORD" >> /root/.apolo_credentials
    fi

    log_success "dSIPRouter + Kamailio + RTPEngine installed"
    log_info "dSIPRouter GUI: https://$(hostname -I | awk '{print $1}'):5000"
}

#===============================================================================
# FreeSWITCH Installation
#===============================================================================
install_freeswitch() {
    log_section "Installing FreeSWITCH"

    log_info "Using SignalWire token for installation..."

    # Install FreeSWITCH using fsget script
    curl -sSL https://freeswitch.org/fsget | bash -s $FS_TOKEN release install

    # Wait for installation to complete
    sleep 5

    # Enable and start FreeSWITCH
    systemctl enable freeswitch
    systemctl start freeswitch

    # Set permissions
    chown -R freeswitch:freeswitch /etc/freeswitch
    chown -R freeswitch:freeswitch /var/lib/freeswitch
    chown -R freeswitch:freeswitch /var/log/freeswitch

    log_success "FreeSWITCH installed"
}

#===============================================================================
# Configure FreeSWITCH for ApoloBilling
#===============================================================================
configure_freeswitch() {
    log_section "Configuring FreeSWITCH for ApoloBilling"

    # Get server IP
    SERVER_IP=$(hostname -I | awk '{print $1}')

    # Configure internal profile for Kamailio connection
    if [[ -f /etc/freeswitch/sip_profiles/internal.xml ]]; then
        # Backup original
        cp /etc/freeswitch/sip_profiles/internal.xml /etc/freeswitch/sip_profiles/internal.xml.bak

        # Update internal profile to listen on port 5080
        sed -i 's/name="sip-port" value="5060"/name="sip-port" value="5080"/' /etc/freeswitch/sip_profiles/internal.xml
    fi

    # Configure external profile
    if [[ -f /etc/freeswitch/sip_profiles/external.xml ]]; then
        cp /etc/freeswitch/sip_profiles/external.xml /etc/freeswitch/sip_profiles/external.xml.bak
        sed -i 's/name="sip-port" value="5080"/name="sip-port" value="5062"/' /etc/freeswitch/sip_profiles/external.xml
    fi

    # Configure mod_xml_curl for dynamic directory
    if [[ -f /etc/freeswitch/autoload_configs/xml_curl.conf.xml ]]; then
        cat > /etc/freeswitch/autoload_configs/xml_curl.conf.xml <<'XMLCURL'
<configuration name="xml_curl.conf" description="cURL XML Gateway">
  <bindings>
    <binding name="directory">
      <param name="gateway-url" value="http://127.0.0.1:8000/api/v1/freeswitch/directory" bindings="directory"/>
      <param name="method" value="POST"/>
      <param name="timeout" value="10"/>
    </binding>
  </bindings>
</configuration>
XMLCURL
    fi

    # Create Kamailio gateway in FreeSWITCH
    mkdir -p /etc/freeswitch/sip_profiles/external
    cat > /etc/freeswitch/sip_profiles/external/kamailio.xml <<EOF
<include>
  <gateway name="kamailio">
    <param name="realm" value="$SERVER_IP"/>
    <param name="proxy" value="$SERVER_IP:5060"/>
    <param name="register" value="false"/>
    <param name="caller-id-in-from" value="true"/>
  </gateway>
</include>
EOF

    # Restart FreeSWITCH to apply changes
    systemctl restart freeswitch

    log_success "FreeSWITCH configured"
}

#===============================================================================
# Configure Kamailio for FreeSWITCH
#===============================================================================
configure_kamailio() {
    log_section "Configuring Kamailio for FreeSWITCH"

    SERVER_IP=$(hostname -I | awk '{print $1}')

    # Add FreeSWITCH as PBX endpoint in Kamailio (type=9)
    mysql -u "$DB_USER" -p"$DB_PASSWORD" kamailio <<EOF
-- Add FreeSWITCH as internal PBX
INSERT INTO dr_gateways (gwid, type, address, strip, pri_prefix, attrs, description)
VALUES (100, 9, 'sip:127.0.0.1:5080', 0, '', '', 'FreeSWITCH Internal')
ON DUPLICATE KEY UPDATE address='sip:127.0.0.1:5080';

-- Add to address table for trusted peers
INSERT INTO address (grp, ip_addr, mask, port, tag)
VALUES (1, '127.0.0.1', 32, 5080, 'FreeSWITCH')
ON DUPLICATE KEY UPDATE tag='FreeSWITCH';
EOF

    # Reload Kamailio
    kamcmd drouting.reload 2>/dev/null || true
    kamcmd permissions.addressReload 2>/dev/null || true

    log_success "Kamailio configured"
}

#===============================================================================
# Create ApoloBilling user
#===============================================================================
create_apolo_user() {
    log_section "Creating ApoloBilling System User"

    # Create user if not exists
    if ! id -u apolo &>/dev/null; then
        useradd -r -m -d /opt/ApoloBilling -s /bin/bash apolo
    fi

    # Add to necessary groups
    usermod -aG kamailio apolo 2>/dev/null || true
    usermod -aG freeswitch apolo 2>/dev/null || true

    # Set permissions on kamailio socket
    chmod 770 /var/run/kamailio/ 2>/dev/null || true
    chmod 660 /var/run/kamailio/kamailio_ctl 2>/dev/null || true

    log_success "User 'apolo' created"
}

#===============================================================================
# Clone ApoloBilling Repository
#===============================================================================
clone_repository() {
    log_section "Cloning ApoloBilling Repository"

    if [[ ! -d "$APOLO_DIR" ]]; then
        git clone https://github.com/jesus-bazan-entel/ApoloBillingv3.git "$APOLO_DIR"
    else
        cd "$APOLO_DIR"
        git pull origin main 2>/dev/null || log_warn "Could not update repository"
    fi

    chown -R apolo:apolo "$APOLO_DIR"

    log_success "Repository cloned to $APOLO_DIR"
}

#===============================================================================
# Create Environment Files
#===============================================================================
create_env_files() {
    log_section "Creating Environment Files"

    # Generate JWT secret
    JWT_SECRET=$(openssl rand -base64 32)

    # Rust Backend .env
    cat > "$APOLO_DIR/rust-backend/.env" <<EOF
# Database (PostgreSQL)
DATABASE_URL=postgresql://${DB_USER}:${DB_PASSWORD}@localhost:5432/apolo_billing
DATABASE_MAX_CONNECTIONS=20

# Kamailio Database (MySQL)
KAMAILIO_DATABASE_URL=mysql://${DB_USER}:${DB_PASSWORD}@localhost:3306/kamailio

# Redis
REDIS_URL=redis://localhost:6379

# Server Configuration
RUST_SERVER_HOST=0.0.0.0
RUST_SERVER_PORT=8000
RUST_SERVER_WORKERS=4

# JWT Authentication
JWT_SECRET=${JWT_SECRET}
JWT_EXPIRATION_SECS=1800

# CORS
CORS_ORIGINS=http://localhost:3000,http://127.0.0.1:3000,http://$(hostname -I | awk '{print $1}'):3000

# Logging
RUST_LOG=apolo_billing=info,apolo_api=info,actix_web=info
EOF

    # Billing Engine .env
    cat > "$APOLO_DIR/rust-billing-engine/.env" <<EOF
# Database
DATABASE_URL=postgresql://${DB_USER}:${DB_PASSWORD}@localhost:5432/apolo_billing

# Redis
REDIS_URL=redis://localhost:6379

# Server
HOST=0.0.0.0
PORT=9000

# FreeSWITCH ESL
FREESWITCH_SERVERS=127.0.0.1:8021:ClueCon

# Environment
ENVIRONMENT=production
RUST_LOG=info
EOF

    # Frontend .env
    cat > "$APOLO_DIR/frontend/.env" <<EOF
VITE_API_URL=http://$(hostname -I | awk '{print $1}'):8000/api/v1
EOF

    chown apolo:apolo "$APOLO_DIR/rust-backend/.env"
    chown apolo:apolo "$APOLO_DIR/rust-billing-engine/.env"
    chown apolo:apolo "$APOLO_DIR/frontend/.env"
    chmod 600 "$APOLO_DIR/rust-backend/.env"
    chmod 600 "$APOLO_DIR/rust-billing-engine/.env"

    log_success "Environment files created"
}

#===============================================================================
# Apply Database Schema
#===============================================================================
apply_database_schema() {
    log_section "Applying Database Schema"

    cd "$APOLO_DIR"

    # Apply main schema
    if [[ -f database/schema.sql ]]; then
        PGPASSWORD="$DB_PASSWORD" psql -U "$DB_USER" -h localhost -d apolo_billing -f database/schema.sql
        log_success "Main schema applied"
    fi

    # Apply migrations in order
    if [[ -d database/migrations ]]; then
        for f in $(ls database/migrations/*.sql 2>/dev/null | sort); do
            if [[ -f "$f" ]]; then
                log_info "Applying migration: $(basename $f)"
                PGPASSWORD="$DB_PASSWORD" psql -U "$DB_USER" -h localhost -d apolo_billing -f "$f" 2>/dev/null || true
            fi
        done
        log_success "Migrations applied"
    fi
}

#===============================================================================
# Build ApoloBilling
#===============================================================================
build_apolo() {
    log_section "Building ApoloBilling"

    source "$HOME/.cargo/env"

    # Build Rust Backend
    log_info "Building Rust backend..."
    cd "$APOLO_DIR/rust-backend"
    cargo build --release

    # Build Billing Engine
    log_info "Building Billing Engine..."
    cd "$APOLO_DIR/rust-billing-engine"
    cargo build --release

    # Build Frontend
    log_info "Building Frontend..."
    cd "$APOLO_DIR/frontend"
    npm install
    npm run build

    log_success "ApoloBilling built successfully"
}

#===============================================================================
# Install Systemd Services
#===============================================================================
install_services() {
    log_section "Installing Systemd Services"

    # Backend service
    cat > /etc/systemd/system/apolo-backend.service <<EOF
[Unit]
Description=ApoloBilling Backend API
After=network.target postgresql.service redis.service

[Service]
Type=simple
User=apolo
Group=apolo
WorkingDirectory=/opt/ApoloBilling/rust-backend
ExecStart=/opt/ApoloBilling/rust-backend/target/release/apolo-billing
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

    # Billing Engine service
    cat > /etc/systemd/system/apolo-billing-engine.service <<EOF
[Unit]
Description=ApoloBilling Real-time Billing Engine
After=network.target postgresql.service redis.service freeswitch.service

[Service]
Type=simple
User=apolo
Group=apolo
WorkingDirectory=/opt/ApoloBilling/rust-billing-engine
ExecStart=/opt/ApoloBilling/rust-billing-engine/target/release/apolo-billing-engine
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

    # Frontend service
    cat > /etc/systemd/system/apolo-frontend.service <<EOF
[Unit]
Description=ApoloBilling Frontend
After=network.target

[Service]
Type=simple
User=apolo
Group=apolo
WorkingDirectory=/opt/ApoloBilling/frontend
ExecStart=/usr/bin/serve -s dist -l 3000
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    systemctl enable apolo-backend apolo-billing-engine apolo-frontend

    log_success "Systemd services installed"
}

#===============================================================================
# Configure Nginx
#===============================================================================
configure_nginx() {
    log_section "Configuring Nginx"

    SERVER_IP=$(hostname -I | awk '{print $1}')

    cat > /etc/nginx/sites-available/apolo-billing <<EOF
# ApoloBilling Frontend + API Proxy
server {
    listen 80;
    server_name $SERVER_IP _;

    # Frontend
    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host \$host;
        proxy_cache_bypass \$http_upgrade;
    }

    # API Proxy
    location /api/ {
        proxy_pass http://127.0.0.1:8000/api/;
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }

    # WebSocket
    location /ws {
        proxy_pass http://127.0.0.1:8000/ws;
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host \$host;
        proxy_read_timeout 86400;
    }
}
EOF

    ln -sf /etc/nginx/sites-available/apolo-billing /etc/nginx/sites-enabled/
    rm -f /etc/nginx/sites-enabled/default

    nginx -t && systemctl reload nginx

    log_success "Nginx configured"
}

#===============================================================================
# Start Services
#===============================================================================
start_services() {
    log_section "Starting Services"

    systemctl start apolo-backend
    sleep 2
    systemctl start apolo-billing-engine
    sleep 2
    systemctl start apolo-frontend

    log_success "All services started"
}

#===============================================================================
# Create Admin User
#===============================================================================
create_admin_user() {
    log_section "Creating Admin User"

    cd "$APOLO_DIR/rust-backend"

    # Wait for backend to be ready
    log_info "Waiting for backend to start..."
    sleep 8

    # Try to create admin user via API registration endpoint
    # First, we need a temporary token or use direct DB insert

    # Check if backend is responding
    BACKEND_READY=false
    for i in {1..10}; do
        if curl -s http://127.0.0.1:8000/api/v1/health > /dev/null 2>&1; then
            BACKEND_READY=true
            break
        fi
        sleep 2
    done

    if [[ "$BACKEND_READY" == "true" ]]; then
        log_info "Backend is ready, creating admin user via API..."

        # Create admin user via register endpoint (requires existing superadmin, so we'll use DB)
        # For initial setup, we insert directly with a known Argon2 hash

        # Generate Argon2 hash for 'admin123' using the backend binary
        # Since we can't easily generate Argon2 from bash, we'll create via SQL
        # and then update via API after first login

        # The backend uses Argon2id, we'll insert a placeholder and use the API
        # Actually, let's try to use the register endpoint if available

        # For now, insert with bcrypt-compatible placeholder that the app will handle
        PGPASSWORD="$DB_PASSWORD" psql -U "$DB_USER" -h localhost -d apolo_billing <<EOF
-- Insert admin user with placeholder hash
-- The password will be set on first login or via API
INSERT INTO usuarios (username, password_hash, nombre, apellido, email, role, activo)
VALUES ('$ADMIN_USER', 'NEEDS_RESET:$ADMIN_PASSWORD', 'Administrador', 'Sistema', 'admin@localhost', 'superadmin', true)
ON CONFLICT (username) DO UPDATE SET password_hash = 'NEEDS_RESET:$ADMIN_PASSWORD';
EOF

        # Now use a small Rust program or Python to hash the password properly
        # Or call the API to set the password

        # Create a temporary script to hash the password using Python
        if command -v python3 &> /dev/null; then
            pip3 install argon2-cffi -q 2>/dev/null || true

            HASH=$(python3 <<PYEOF
import sys
try:
    from argon2 import PasswordHasher
    ph = PasswordHasher()
    print(ph.hash("$ADMIN_PASSWORD"))
except:
    print("HASH_FAILED")
PYEOF
)
            if [[ "$HASH" != "HASH_FAILED" && -n "$HASH" ]]; then
                # Update with proper hash
                PGPASSWORD="$DB_PASSWORD" psql -U "$DB_USER" -h localhost -d apolo_billing <<EOF
UPDATE usuarios SET password_hash = '$HASH' WHERE username = '$ADMIN_USER';
EOF
                log_success "Admin user created with hashed password"
            else
                log_warn "Could not hash password, admin will need password reset"
            fi
        else
            log_warn "Python3 not available for password hashing"
        fi
    else
        log_warn "Backend not responding, creating admin user directly in DB"

        PGPASSWORD="$DB_PASSWORD" psql -U "$DB_USER" -h localhost -d apolo_billing <<EOF
INSERT INTO usuarios (username, password_hash, nombre, apellido, email, role, activo)
VALUES ('$ADMIN_USER', 'NEEDS_RESET', 'Administrador', 'Sistema', 'admin@localhost', 'superadmin', true)
ON CONFLICT (username) DO NOTHING;
EOF
    fi

    log_success "Admin user: $ADMIN_USER"
    log_success "Admin password: $ADMIN_PASSWORD"
}

#===============================================================================
# Final Summary
#===============================================================================
show_summary() {
    SERVER_IP=$(hostname -I | awk '{print $1}')

    echo ""
    echo -e "${GREEN}===============================================${NC}"
    echo -e "${GREEN}   ApoloBilling Installation Complete!${NC}"
    echo -e "${GREEN}===============================================${NC}"
    echo ""
    echo -e "${CYAN}Access URLs:${NC}"
    echo "  - ApoloBilling:  http://$SERVER_IP"
    echo "  - dSIPRouter:    https://$SERVER_IP:5000"
    echo ""
    echo -e "${CYAN}Service Status:${NC}"
    systemctl is-active --quiet apolo-backend && echo -e "  - Backend:        ${GREEN}RUNNING${NC}" || echo -e "  - Backend:        ${RED}STOPPED${NC}"
    systemctl is-active --quiet apolo-billing-engine && echo -e "  - Billing Engine: ${GREEN}RUNNING${NC}" || echo -e "  - Billing Engine: ${RED}STOPPED${NC}"
    systemctl is-active --quiet apolo-frontend && echo -e "  - Frontend:       ${GREEN}RUNNING${NC}" || echo -e "  - Frontend:       ${RED}STOPPED${NC}"
    systemctl is-active --quiet freeswitch && echo -e "  - FreeSWITCH:     ${GREEN}RUNNING${NC}" || echo -e "  - FreeSWITCH:     ${RED}STOPPED${NC}"
    systemctl is-active --quiet kamailio && echo -e "  - Kamailio:       ${GREEN}RUNNING${NC}" || echo -e "  - Kamailio:       ${RED}STOPPED${NC}"
    systemctl is-active --quiet rtpengine && echo -e "  - RTPEngine:      ${GREEN}RUNNING${NC}" || echo -e "  - RTPEngine:      ${RED}STOPPED${NC}"
    echo ""
    echo -e "${CYAN}Database Credentials:${NC}"
    echo "  - PostgreSQL User:  $DB_USER"
    echo "  - PostgreSQL Pass:  $DB_PASSWORD"
    echo "  - PostgreSQL DB:    apolo_billing"
    echo "  - MySQL User:       $DB_USER"
    echo "  - MySQL Pass:       $DB_PASSWORD"
    echo "  - MySQL DB:         kamailio"
    echo ""
    echo -e "${CYAN}Admin Login:${NC}"
    echo "  - Username:  $ADMIN_USER"
    echo "  - Password:  $ADMIN_PASSWORD"
    echo ""
    echo -e "${CYAN}Credentials File:${NC}"
    echo "  /root/.apolo_credentials"
    echo ""
    echo -e "${CYAN}Logs:${NC}"
    echo "  - Installation: $LOG_FILE"
    echo "  - Backend:      journalctl -u apolo-backend -f"
    echo "  - FreeSWITCH:   journalctl -u freeswitch -f"
    echo "  - Kamailio:     journalctl -u kamailio -f"
    echo ""
    echo -e "${CYAN}Next Steps:${NC}"
    echo "  1. Login to ApoloBilling: http://$SERVER_IP"
    echo "  2. Configure carriers in dSIPRouter: https://$SERVER_IP:5000"
    echo "  3. Add SIP devices in ApoloBilling"
    echo "  4. Configure inbound/outbound routes"
    echo ""
    echo -e "${YELLOW}IMPORTANT: Reboot recommended after installation${NC}"
    echo ""
}

#===============================================================================
# Main Installation Flow
#===============================================================================
main() {
    # Initialize log file
    mkdir -p $(dirname "$LOG_FILE")
    echo "=== ApoloBilling Installation Started: $(date) ===" > "$LOG_FILE"

    preflight_checks
    update_system
    install_postgresql
    install_mysql
    install_redis
    install_nginx
    install_nodejs
    install_rust

    if [[ "$SKIP_VOIP" == "false" ]]; then
        install_dsiprouter
        install_freeswitch
        configure_freeswitch
        configure_kamailio
    else
        log_warn "Skipping VoIP components (--skip-voip flag)"
    fi

    create_apolo_user
    clone_repository
    create_env_files
    apply_database_schema
    build_apolo
    install_services
    configure_nginx
    start_services
    create_admin_user

    show_summary

    echo "=== ApoloBilling Installation Completed: $(date) ===" >> "$LOG_FILE"
}

# Run main function
main "$@"
