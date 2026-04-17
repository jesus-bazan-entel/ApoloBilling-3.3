#!/bin/bash
#===============================================================================
# ApoloBilling - Backup & Restore Script
# Version: 1.0.0
#
# Usage:
#   ./backup_restore.sh backup              - Create full system backup
#   ./backup_restore.sh restore <file>      - Restore from backup file
#   ./backup_restore.sh install-fresh       - Fresh install on new server
#===============================================================================

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
BACKUP_DIR="/opt/ApoloBilling/backups"
APOLO_DIR="/opt/ApoloBilling"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="apolo_backup_${TIMESTAMP}.tar.gz"

# Database credentials (will be read from .env)
PG_USER=""
PG_PASS=""
PG_DB="apolo_billing"
MYSQL_USER=""
MYSQL_PASS=""
MYSQL_DB="kamailio"

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

#===============================================================================
# Load credentials from .env files
#===============================================================================
load_credentials() {
    log_info "Loading credentials from .env files..."

    if [[ -f "$APOLO_DIR/rust-backend/.env" ]]; then
        # PostgreSQL
        PG_URL=$(grep "^DATABASE_URL" "$APOLO_DIR/rust-backend/.env" | cut -d'=' -f2-)
        if [[ -n "$PG_URL" ]]; then
            PG_USER=$(echo "$PG_URL" | sed -n 's|.*://\([^:]*\):.*|\1|p')
            PG_PASS=$(echo "$PG_URL" | sed -n 's|.*://[^:]*:\([^@]*\)@.*|\1|p')
        fi

        # MySQL/Kamailio
        MYSQL_URL=$(grep "^KAMAILIO_DATABASE_URL" "$APOLO_DIR/rust-backend/.env" | cut -d'=' -f2-)
        if [[ -n "$MYSQL_URL" ]]; then
            MYSQL_USER=$(echo "$MYSQL_URL" | sed -n 's|.*://\([^:]*\):.*|\1|p')
            MYSQL_PASS=$(echo "$MYSQL_URL" | sed -n 's|.*://[^:]*:\([^@]*\)@.*|\1|p')
        fi
    fi

    log_success "Credentials loaded"
}

#===============================================================================
# BACKUP FUNCTION
#===============================================================================
do_backup() {
    log_info "Starting ApoloBilling full backup..."

    mkdir -p "$BACKUP_DIR"
    WORK_DIR=$(mktemp -d)

    load_credentials

    # 1. Backup PostgreSQL database
    log_info "Backing up PostgreSQL database..."
    PGPASSWORD="$PG_PASS" pg_dump -U "$PG_USER" -h localhost "$PG_DB" > "$WORK_DIR/postgresql_apolo_billing.sql" 2>/dev/null || {
        log_warn "PostgreSQL backup failed, trying with sudo..."
        sudo -u postgres pg_dump "$PG_DB" > "$WORK_DIR/postgresql_apolo_billing.sql"
    }
    log_success "PostgreSQL backup complete"

    # 2. Backup MySQL/Kamailio database
    log_info "Backing up Kamailio MySQL database..."
    if [[ -n "$MYSQL_PASS" ]]; then
        mysqldump -u "$MYSQL_USER" -p"$MYSQL_PASS" "$MYSQL_DB" > "$WORK_DIR/mysql_kamailio.sql" 2>/dev/null || log_warn "MySQL backup failed"
    else
        log_warn "MySQL credentials not found, skipping Kamailio backup"
    fi

    # 3. Backup configuration files
    log_info "Backing up configuration files..."
    mkdir -p "$WORK_DIR/config"

    # .env files
    cp "$APOLO_DIR/rust-backend/.env" "$WORK_DIR/config/backend.env" 2>/dev/null || true
    cp "$APOLO_DIR/rust-billing-engine/.env" "$WORK_DIR/config/billing-engine.env" 2>/dev/null || true
    cp "$APOLO_DIR/frontend/.env" "$WORK_DIR/config/frontend.env" 2>/dev/null || true

    # Systemd services
    mkdir -p "$WORK_DIR/config/systemd"
    cp /etc/systemd/system/apolo*.service "$WORK_DIR/config/systemd/" 2>/dev/null || true

    # Nginx config
    mkdir -p "$WORK_DIR/config/nginx"
    cp /etc/nginx/sites-available/apolo* "$WORK_DIR/config/nginx/" 2>/dev/null || true
    cp /etc/nginx/sites-enabled/apolo* "$WORK_DIR/config/nginx/" 2>/dev/null || true

    # FreeSWITCH - Full configuration directory
    log_info "Backing up FreeSWITCH configuration..."
    if [[ -d "/etc/freeswitch" ]]; then
        cp -r /etc/freeswitch "$WORK_DIR/config/freeswitch"
        log_success "FreeSWITCH config backed up"
    else
        log_warn "FreeSWITCH config directory not found"
    fi

    # FreeSWITCH - Scripts and sounds (optional, can be large)
    if [[ -d "/usr/share/freeswitch/scripts" ]]; then
        mkdir -p "$WORK_DIR/config/freeswitch-scripts"
        cp -r /usr/share/freeswitch/scripts "$WORK_DIR/config/freeswitch-scripts/" 2>/dev/null || true
    fi

    # Kamailio - Full configuration directory
    log_info "Backing up Kamailio configuration..."
    if [[ -d "/etc/kamailio" ]]; then
        cp -r /etc/kamailio "$WORK_DIR/config/kamailio"
        log_success "Kamailio config backed up"
    else
        log_warn "Kamailio config directory not found"
    fi

    # dSIPRouter configuration (if exists)
    if [[ -d "/etc/dsiprouter" ]]; then
        log_info "Backing up dSIPRouter configuration..."
        cp -r /etc/dsiprouter "$WORK_DIR/config/dsiprouter"
        log_success "dSIPRouter config backed up"
    fi

    # dSIPRouter installation directory (settings and GUI)
    if [[ -d "/opt/dsiprouter" ]]; then
        log_info "Backing up dSIPRouter settings..."
        mkdir -p "$WORK_DIR/config/dsiprouter-opt"
        cp /opt/dsiprouter/gui/settings.py "$WORK_DIR/config/dsiprouter-opt/" 2>/dev/null || true
        cp /opt/dsiprouter/gui/dsiprouter.py "$WORK_DIR/config/dsiprouter-opt/" 2>/dev/null || true
        cp -r /opt/dsiprouter/gui/modules "$WORK_DIR/config/dsiprouter-opt/" 2>/dev/null || true
    fi

    # RTPEngine configuration
    log_info "Backing up RTPEngine configuration..."
    if [[ -d "/etc/rtpengine" ]]; then
        cp -r /etc/rtpengine "$WORK_DIR/config/rtpengine"
        log_success "RTPEngine config backed up"
    elif [[ -f "/etc/default/rtpengine" ]]; then
        mkdir -p "$WORK_DIR/config/rtpengine"
        cp /etc/default/rtpengine "$WORK_DIR/config/rtpengine/"
        log_success "RTPEngine config backed up"
    else
        log_warn "RTPEngine config not found"
    fi

    # Additional systemd services (FreeSWITCH, Kamailio, RTPEngine)
    log_info "Backing up additional systemd services..."
    cp /etc/systemd/system/freeswitch*.service "$WORK_DIR/config/systemd/" 2>/dev/null || true
    cp /etc/systemd/system/kamailio*.service "$WORK_DIR/config/systemd/" 2>/dev/null || true
    cp /etc/systemd/system/rtpengine*.service "$WORK_DIR/config/systemd/" 2>/dev/null || true
    cp /etc/systemd/system/dsiprouter*.service "$WORK_DIR/config/systemd/" 2>/dev/null || true
    cp /lib/systemd/system/freeswitch.service "$WORK_DIR/config/systemd/freeswitch-lib.service" 2>/dev/null || true
    cp /lib/systemd/system/kamailio.service "$WORK_DIR/config/systemd/kamailio-lib.service" 2>/dev/null || true

    log_success "Configuration backup complete"

    # 4. Backup compiled binaries
    log_info "Backing up compiled binaries..."
    mkdir -p "$WORK_DIR/binaries"
    cp "$APOLO_DIR/rust-backend/target/release/apolo-billing" "$WORK_DIR/binaries/" 2>/dev/null || true
    cp "$APOLO_DIR/rust-billing-engine/target/release/apolo-billing-engine" "$WORK_DIR/binaries/" 2>/dev/null || true

    # 5. Backup frontend dist
    log_info "Backing up frontend build..."
    if [[ -d "$APOLO_DIR/frontend/dist" ]]; then
        cp -r "$APOLO_DIR/frontend/dist" "$WORK_DIR/frontend_dist"
    fi

    # 6. Backup source code reference (git info)
    log_info "Saving git information..."
    cd "$APOLO_DIR"
    git rev-parse HEAD > "$WORK_DIR/git_commit.txt" 2>/dev/null || echo "unknown" > "$WORK_DIR/git_commit.txt"
    git remote -v > "$WORK_DIR/git_remote.txt" 2>/dev/null || true

    # 7. Create metadata file
    cat > "$WORK_DIR/backup_metadata.json" <<EOF
{
    "timestamp": "$(date -Iseconds)",
    "hostname": "$(hostname)",
    "backup_version": "2.0.0",
    "git_commit": "$(cat $WORK_DIR/git_commit.txt)",
    "components": {
        "postgresql": true,
        "mysql": $([ -f "$WORK_DIR/mysql_kamailio.sql" ] && echo "true" || echo "false"),
        "binaries": true,
        "frontend": $([ -d "$WORK_DIR/frontend_dist" ] && echo "true" || echo "false"),
        "freeswitch": $([ -d "$WORK_DIR/config/freeswitch" ] && echo "true" || echo "false"),
        "kamailio": $([ -d "$WORK_DIR/config/kamailio" ] && echo "true" || echo "false"),
        "rtpengine": $([ -d "$WORK_DIR/config/rtpengine" ] && echo "true" || echo "false"),
        "dsiprouter": $([ -d "$WORK_DIR/config/dsiprouter" ] && echo "true" || echo "false"),
        "config": true
    }
}
EOF

    # 8. Create compressed archive
    log_info "Creating compressed backup archive..."
    cd "$WORK_DIR"
    tar -czf "$BACKUP_DIR/$BACKUP_FILE" .

    # Cleanup
    rm -rf "$WORK_DIR"

    BACKUP_SIZE=$(du -h "$BACKUP_DIR/$BACKUP_FILE" | cut -f1)

    echo ""
    log_success "==========================================="
    log_success "Backup completed successfully!"
    log_success "==========================================="
    echo ""
    echo -e "Backup file: ${GREEN}$BACKUP_DIR/$BACKUP_FILE${NC}"
    echo -e "Size: ${GREEN}$BACKUP_SIZE${NC}"
    echo ""
    echo "Components included:"
    echo "  - PostgreSQL (apolo_billing database)"
    [[ -f "$BACKUP_DIR/../mysql_check" ]] || echo "  - MySQL (kamailio database)"
    echo "  - ApoloBilling binaries and frontend"
    echo "  - FreeSWITCH configuration (/etc/freeswitch/)"
    echo "  - Kamailio configuration (/etc/kamailio/)"
    echo "  - RTPEngine configuration"
    echo "  - Systemd services"
    echo "  - Nginx configuration"
    echo ""
    echo "To restore on a new server:"
    echo "  1. Install base packages: FreeSWITCH, Kamailio, RTPEngine, PostgreSQL, MySQL"
    echo "  2. Copy backup file to new server:"
    echo "     scp $BACKUP_DIR/$BACKUP_FILE root@new-server:/tmp/"
    echo "  3. Run restore:"
    echo "     ./backup_restore.sh restore /tmp/$BACKUP_FILE"
}

#===============================================================================
# RESTORE FUNCTION
#===============================================================================
do_restore() {
    local BACKUP_PATH="$1"

    if [[ ! -f "$BACKUP_PATH" ]]; then
        # Try in backup directory
        if [[ -f "$BACKUP_DIR/$BACKUP_PATH" ]]; then
            BACKUP_PATH="$BACKUP_DIR/$BACKUP_PATH"
        else
            log_error "Backup file not found: $BACKUP_PATH"
            exit 1
        fi
    fi

    log_info "Starting ApoloBilling restore from: $BACKUP_PATH"

    WORK_DIR=$(mktemp -d)

    # Extract backup
    log_info "Extracting backup..."
    tar -xzf "$BACKUP_PATH" -C "$WORK_DIR"

    # Show metadata
    if [[ -f "$WORK_DIR/backup_metadata.json" ]]; then
        echo ""
        log_info "Backup metadata:"
        cat "$WORK_DIR/backup_metadata.json"
        echo ""
    fi

    # Confirm restoration
    read -p "Continue with restoration? (yes/no): " CONFIRM
    if [[ "$CONFIRM" != "yes" ]]; then
        log_warn "Restoration cancelled"
        rm -rf "$WORK_DIR"
        exit 0
    fi

    # 1. Restore PostgreSQL
    if [[ -f "$WORK_DIR/postgresql_apolo_billing.sql" ]]; then
        log_info "Restoring PostgreSQL database..."

        # Check if database exists
        if sudo -u postgres psql -lqt | cut -d \| -f 1 | grep -qw "$PG_DB"; then
            log_warn "Database $PG_DB exists, dropping and recreating..."
            sudo -u postgres psql -c "DROP DATABASE IF EXISTS $PG_DB;"
        fi

        sudo -u postgres psql -c "CREATE DATABASE $PG_DB;" 2>/dev/null || true
        sudo -u postgres psql "$PG_DB" < "$WORK_DIR/postgresql_apolo_billing.sql"
        log_success "PostgreSQL restored"
    fi

    # 2. Restore MySQL/Kamailio
    if [[ -f "$WORK_DIR/mysql_kamailio.sql" ]]; then
        log_info "Restoring Kamailio MySQL database..."

        if [[ -f "$WORK_DIR/config/backend.env" ]]; then
            MYSQL_URL=$(grep "^KAMAILIO_DATABASE_URL" "$WORK_DIR/config/backend.env" | cut -d'=' -f2-)
            MYSQL_USER=$(echo "$MYSQL_URL" | sed -n 's|.*://\([^:]*\):.*|\1|p')
            MYSQL_PASS=$(echo "$MYSQL_URL" | sed -n 's|.*://[^:]*:\([^@]*\)@.*|\1|p')

            mysql -u "$MYSQL_USER" -p"$MYSQL_PASS" "$MYSQL_DB" < "$WORK_DIR/mysql_kamailio.sql" || log_warn "MySQL restore failed"
        fi
        log_success "MySQL restored"
    fi

    # 3. Restore configuration files
    log_info "Restoring configuration files..."

    mkdir -p "$APOLO_DIR/rust-backend"
    mkdir -p "$APOLO_DIR/rust-billing-engine"
    mkdir -p "$APOLO_DIR/frontend"

    cp "$WORK_DIR/config/backend.env" "$APOLO_DIR/rust-backend/.env" 2>/dev/null || true
    cp "$WORK_DIR/config/billing-engine.env" "$APOLO_DIR/rust-billing-engine/.env" 2>/dev/null || true
    cp "$WORK_DIR/config/frontend.env" "$APOLO_DIR/frontend/.env" 2>/dev/null || true

    # Systemd services
    if [[ -d "$WORK_DIR/config/systemd" ]]; then
        cp "$WORK_DIR/config/systemd/"*.service /etc/systemd/system/ 2>/dev/null || true
        systemctl daemon-reload
    fi

    log_success "ApoloBilling configuration restored"

    # 3b. Restore FreeSWITCH configuration
    if [[ -d "$WORK_DIR/config/freeswitch" ]]; then
        log_info "Restoring FreeSWITCH configuration..."

        # Backup existing config if present
        if [[ -d "/etc/freeswitch" ]]; then
            mv /etc/freeswitch /etc/freeswitch.bak.$(date +%Y%m%d_%H%M%S) 2>/dev/null || true
        fi

        cp -r "$WORK_DIR/config/freeswitch" /etc/freeswitch
        chown -R freeswitch:freeswitch /etc/freeswitch 2>/dev/null || true
        log_success "FreeSWITCH configuration restored"
    fi

    # Restore FreeSWITCH scripts
    if [[ -d "$WORK_DIR/config/freeswitch-scripts" ]]; then
        log_info "Restoring FreeSWITCH scripts..."
        cp -r "$WORK_DIR/config/freeswitch-scripts/"* /usr/share/freeswitch/scripts/ 2>/dev/null || true
    fi

    # 3c. Restore Kamailio configuration
    if [[ -d "$WORK_DIR/config/kamailio" ]]; then
        log_info "Restoring Kamailio configuration..."

        # Backup existing config if present
        if [[ -d "/etc/kamailio" ]]; then
            mv /etc/kamailio /etc/kamailio.bak.$(date +%Y%m%d_%H%M%S) 2>/dev/null || true
        fi

        cp -r "$WORK_DIR/config/kamailio" /etc/kamailio
        chown -R kamailio:kamailio /etc/kamailio 2>/dev/null || true
        log_success "Kamailio configuration restored"
    fi

    # 3d. Restore dSIPRouter configuration
    if [[ -d "$WORK_DIR/config/dsiprouter" ]]; then
        log_info "Restoring dSIPRouter configuration..."
        cp -r "$WORK_DIR/config/dsiprouter" /etc/dsiprouter 2>/dev/null || true
        log_success "dSIPRouter configuration restored"
    fi

    if [[ -d "$WORK_DIR/config/dsiprouter-opt" ]]; then
        log_info "Restoring dSIPRouter settings..."
        mkdir -p /opt/dsiprouter/gui
        cp "$WORK_DIR/config/dsiprouter-opt/settings.py" /opt/dsiprouter/gui/ 2>/dev/null || true
        log_success "dSIPRouter settings restored"
    fi

    # 3e. Restore RTPEngine configuration
    if [[ -d "$WORK_DIR/config/rtpengine" ]]; then
        log_info "Restoring RTPEngine configuration..."

        if [[ -f "$WORK_DIR/config/rtpengine/rtpengine" ]]; then
            cp "$WORK_DIR/config/rtpengine/rtpengine" /etc/default/rtpengine 2>/dev/null || true
        fi

        if [[ -d "/etc/rtpengine" ]]; then
            cp -r "$WORK_DIR/config/rtpengine/"* /etc/rtpengine/ 2>/dev/null || true
        else
            mkdir -p /etc/rtpengine
            cp -r "$WORK_DIR/config/rtpengine/"* /etc/rtpengine/ 2>/dev/null || true
        fi

        log_success "RTPEngine configuration restored"
    fi

    # 4. Restore binaries
    if [[ -d "$WORK_DIR/binaries" ]]; then
        log_info "Restoring binaries..."
        mkdir -p "$APOLO_DIR/rust-backend/target/release"
        mkdir -p "$APOLO_DIR/rust-billing-engine/target/release"

        cp "$WORK_DIR/binaries/apolo-billing" "$APOLO_DIR/rust-backend/target/release/" 2>/dev/null || true
        cp "$WORK_DIR/binaries/apolo-billing-engine" "$APOLO_DIR/rust-billing-engine/target/release/" 2>/dev/null || true

        chmod +x "$APOLO_DIR/rust-backend/target/release/apolo-billing" 2>/dev/null || true
        chmod +x "$APOLO_DIR/rust-billing-engine/target/release/apolo-billing-engine" 2>/dev/null || true

        log_success "Binaries restored"
    fi

    # 5. Restore frontend
    if [[ -d "$WORK_DIR/frontend_dist" ]]; then
        log_info "Restoring frontend..."
        mkdir -p "$APOLO_DIR/frontend"
        cp -r "$WORK_DIR/frontend_dist" "$APOLO_DIR/frontend/dist"
        log_success "Frontend restored"
    fi

    # Cleanup
    rm -rf "$WORK_DIR"

    echo ""
    log_success "==========================================="
    log_success "Restore completed successfully!"
    log_success "==========================================="
    echo ""
    echo "Next steps:"
    echo ""
    echo "  1. Verify configuration files:"
    echo "     - $APOLO_DIR/rust-backend/.env"
    echo "     - /etc/freeswitch/ (check IPs and domains)"
    echo "     - /etc/kamailio/kamailio.cfg (check IPs)"
    echo ""
    echo "  2. Update IP addresses if server IP changed:"
    echo "     - FreeSWITCH sip_profiles (internal.xml, external.xml)"
    echo "     - Kamailio listen addresses"
    echo "     - RTPEngine interfaces"
    echo ""
    echo "  3. Restart VoIP services:"
    echo "     systemctl restart freeswitch"
    echo "     systemctl restart kamailio"
    echo "     systemctl restart rtpengine"
    echo ""
    echo "  4. Reload Kamailio routing tables:"
    echo "     kamcmd drouting.reload"
    echo "     kamcmd permissions.addressReload"
    echo ""
    echo "  5. Start ApoloBilling services:"
    echo "     systemctl start apolo-backend"
    echo "     systemctl start apolo-billing-engine"
    echo "     systemctl start apolo-frontend"
    echo ""
    echo "  6. Check logs:"
    echo "     journalctl -u apolo-backend -f"
    echo "     journalctl -u freeswitch -f"
    echo "     journalctl -u kamailio -f"
}

#===============================================================================
# FRESH INSTALL FUNCTION
#===============================================================================
do_fresh_install() {
    log_info "Starting fresh installation on new server..."

    # Check if running as root
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root for fresh install"
        exit 1
    fi

    # Update system
    log_info "Updating system packages..."
    apt-get update
    apt-get upgrade -y

    # Install dependencies
    log_info "Installing dependencies..."
    apt-get install -y \
        curl wget git build-essential \
        postgresql postgresql-contrib \
        mariadb-server \
        redis-server \
        nginx \
        certbot python3-certbot-nginx \
        ca-certificates gnupg

    # Install Node.js 20
    log_info "Installing Node.js 20..."
    curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
    apt-get install -y nodejs
    npm install -g serve

    # Install Rust
    log_info "Installing Rust..."
    if ! command -v cargo &> /dev/null; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi

    # Clone repository
    log_info "Cloning ApoloBilling repository..."
    if [[ ! -d "$APOLO_DIR" ]]; then
        git clone https://github.com/jesus-bazan-entel/ApoloBillingv3.git "$APOLO_DIR"
    else
        cd "$APOLO_DIR"
        git pull origin main
    fi

    # Setup PostgreSQL
    log_info "Setting up PostgreSQL..."
    PG_PASSWORD=$(openssl rand -base64 32 | tr -dc 'a-zA-Z0-9' | head -c 24)

    sudo -u postgres psql <<EOF
CREATE USER apolo_user WITH PASSWORD '$PG_PASSWORD';
CREATE DATABASE apolo_billing OWNER apolo_user;
GRANT ALL PRIVILEGES ON DATABASE apolo_billing TO apolo_user;
EOF

    # Apply schema and migrations
    log_info "Applying database schema..."
    cd "$APOLO_DIR"
    sudo -u postgres psql -d apolo_billing -f database/schema.sql

    for f in database/migrations/*.sql; do
        sudo -u postgres psql -d apolo_billing -f "$f" 2>/dev/null || true
    done

    # Create .env files
    log_info "Creating configuration files..."

    cat > "$APOLO_DIR/rust-backend/.env" <<EOF
DATABASE_URL=postgresql://apolo_user:${PG_PASSWORD}@localhost:5432/apolo_billing
DATABASE_MAX_CONNECTIONS=20
REDIS_URL=redis://localhost:6379
RUST_SERVER_HOST=0.0.0.0
RUST_SERVER_PORT=8000
RUST_SERVER_WORKERS=4
JWT_SECRET=$(openssl rand -base64 32)
JWT_EXPIRATION_SECS=1800
CORS_ORIGINS=http://localhost:3000,http://127.0.0.1:3000
RUST_LOG=apolo_billing=info,apolo_api=info,actix_web=info
EOF

    cat > "$APOLO_DIR/rust-billing-engine/.env" <<EOF
DATABASE_URL=postgresql://apolo_user:${PG_PASSWORD}@localhost:5432/apolo_billing
REDIS_URL=redis://localhost:6379
HOST=0.0.0.0
PORT=9000
FREESWITCH_SERVERS=
ENVIRONMENT=development
RUST_LOG=info
EOF

    cat > "$APOLO_DIR/frontend/.env" <<EOF
VITE_API_URL=http://localhost:8000/api/v1
EOF

    # Build backend
    log_info "Building Rust backend..."
    cd "$APOLO_DIR/rust-backend"
    source "$HOME/.cargo/env"
    cargo build --release

    # Build billing engine
    log_info "Building billing engine..."
    cd "$APOLO_DIR/rust-billing-engine"
    cargo build --release

    # Build frontend
    log_info "Building frontend..."
    cd "$APOLO_DIR/frontend"
    npm install
    npm run build

    # Install systemd services
    log_info "Installing systemd services..."
    cp "$APOLO_DIR/deploy/systemd/"*.service /etc/systemd/system/
    systemctl daemon-reload

    # Enable and start services
    systemctl enable apolo-backend apolo-billing-engine apolo-frontend
    systemctl start apolo-backend apolo-billing-engine apolo-frontend

    # Setup Nginx
    log_info "Configuring Nginx..."
    cp "$APOLO_DIR/deploy/nginx/apolo-frontend.conf" /etc/nginx/sites-available/
    ln -sf /etc/nginx/sites-available/apolo-frontend.conf /etc/nginx/sites-enabled/
    nginx -t && systemctl reload nginx

    echo ""
    log_success "==========================================="
    log_success "Fresh installation completed!"
    log_success "==========================================="
    echo ""
    echo "Access the application at: http://$(hostname -I | awk '{print $1}'):3000"
    echo ""
    echo "Database credentials saved in: $APOLO_DIR/rust-backend/.env"
    echo ""
    echo "Default admin login:"
    echo "  Username: admin"
    echo "  Password: (check database or create new user)"
    echo ""
    echo "Services status:"
    systemctl status apolo-backend --no-pager | head -5
}

#===============================================================================
# LIST BACKUPS
#===============================================================================
list_backups() {
    log_info "Available backups in $BACKUP_DIR:"
    echo ""
    if [[ -d "$BACKUP_DIR" ]]; then
        ls -lh "$BACKUP_DIR"/*.tar.gz 2>/dev/null || echo "No backups found"
    else
        echo "Backup directory does not exist"
    fi
}

#===============================================================================
# MAIN
#===============================================================================
case "${1:-}" in
    backup)
        do_backup
        ;;
    restore)
        if [[ -z "${2:-}" ]]; then
            log_error "Usage: $0 restore <backup_file>"
            list_backups
            exit 1
        fi
        do_restore "$2"
        ;;
    install-fresh)
        do_fresh_install
        ;;
    list)
        list_backups
        ;;
    *)
        echo "ApoloBilling Backup & Restore Script"
        echo ""
        echo "Usage:"
        echo "  $0 backup              Create full system backup"
        echo "  $0 restore <file>      Restore from backup file"
        echo "  $0 install-fresh       Fresh install on new server"
        echo "  $0 list                List available backups"
        echo ""
        echo "Examples:"
        echo "  $0 backup"
        echo "  $0 restore apolo_backup_20260416_120000.tar.gz"
        echo "  $0 install-fresh"
        ;;
esac
