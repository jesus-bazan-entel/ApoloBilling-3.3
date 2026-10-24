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
#   ./install_server.sh --reset-db   - Wipe apolo_billing public schema before
#                                      reapplying schema.sql (DESTRUCTIVE;
#                                      needed when re-running over a DB left
#                                      in a half-applied state)
#
# Network topology flags (accepted both as --flag=VALUE and env vars):
#   --internal-sip-ip     Our LAN SIP IP (FreeSWITCH + Kamailio FS-face)
#   --external-sip-ip     Our public SIP IP (Kamailio carrier-face)
#   --internal-media-ip   RTPEngine 'internal' interface
#   --external-media-ip   RTPEngine 'external' interface
#   --carrier-sip-ip      Peer PSTN carrier's SIP IP
#   --carrier-media-ip    Peer PSTN carrier's RTP IP
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

# Network configuration — six IPs, split by plane (signaling vs media) and by
# who owns the address (our host vs the carrier peer).
#
#   Ours, signaling:
#     INTERNAL_SIP_IP    LAN IP FreeSWITCH binds on (:5060 clients, :5062 FS-Kam
#                        bus) and Kamailio binds on for its FS-facing listener
#                        (:5080).
#     EXTERNAL_SIP_IP    Public IP Kamailio binds on :5060 toward PSTN carriers.
#
#   Ours, media (RTPEngine):
#     INTERNAL_MEDIA_IP  RTPEngine 'internal' interface (LAN / toward FS).
#                        Usually equal to INTERNAL_SIP_IP.
#     EXTERNAL_MEDIA_IP  RTPEngine 'external' interface (public side toward
#                        carriers). Can differ from EXTERNAL_SIP_IP when the
#                        host has a dedicated media egress.
#
#   Peer (carrier / Integratel):
#     CARRIER_SIP_IP     Carrier's own SIP IP — used in Kamailio dr_gateways
#                        and the `address` ACL (group 2) for trusted signaling.
#     CARRIER_MEDIA_IP   Carrier's own RTP source IP — added to `address`
#                        so RTPEngine / media ACLs trust the carrier's media.
#
# Any of these may be set via env var or --flag; missing values are prompted
# interactively (or default to sensible picks in --non-interactive mode).
INTERNAL_SIP_IP="${INTERNAL_SIP_IP:-}"
EXTERNAL_SIP_IP="${EXTERNAL_SIP_IP:-}"
INTERNAL_MEDIA_IP="${INTERNAL_MEDIA_IP:-}"
EXTERNAL_MEDIA_IP="${EXTERNAL_MEDIA_IP:-}"
CARRIER_SIP_IP="${CARRIER_SIP_IP:-}"
CARRIER_MEDIA_IP="${CARRIER_MEDIA_IP:-}"
NON_INTERACTIVE="${NON_INTERACTIVE:-false}"
# If true, apply_database_schema will wipe the `public` schema before loading
# schema.sql. Required for re-running a broken install. Default: false.
RESET_DB="${RESET_DB:-false}"

# Parse arguments
for arg in "$@"; do
    case $arg in
        --skip-voip)
            SKIP_VOIP=true
            shift
            ;;
        --internal-sip-ip=*)
            INTERNAL_SIP_IP="${arg#*=}"
            shift
            ;;
        --external-sip-ip=*)
            EXTERNAL_SIP_IP="${arg#*=}"
            shift
            ;;
        --internal-media-ip=*)
            INTERNAL_MEDIA_IP="${arg#*=}"
            shift
            ;;
        --external-media-ip=*)
            EXTERNAL_MEDIA_IP="${arg#*=}"
            shift
            ;;
        --carrier-sip-ip=*)
            CARRIER_SIP_IP="${arg#*=}"
            shift
            ;;
        --carrier-media-ip=*)
            CARRIER_MEDIA_IP="${arg#*=}"
            shift
            ;;
        --non-interactive|-y)
            NON_INTERACTIVE=true
            shift
            ;;
        --reset-db)
            RESET_DB=true
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
# Network configuration (internal / external IP for SIP signaling + RTPEngine)
#===============================================================================
# Captures the six IP addresses that drive Kamailio / FreeSWITCH / RTPEngine
# listen + advertise + ACL decisions (see the header comment above for their
# individual meanings), derives INTERNAL_IP_NET (CIDR) from the host routing
# table, and exports a compatibility alias INTERNAL_IP_ADDR consumed by the
# dSIPRouter sub-installer and older code paths below.
configure_network() {
    log_section "Network Configuration"

    # Detect a sensible default LAN IP (source IP of default route).
    local DEFAULT_INTERNAL
    DEFAULT_INTERNAL=$(ip -4 route get 1.1.1.1 2>/dev/null | awk '/src/ {for(i=1;i<=NF;i++) if($i=="src"){print $(i+1); exit}}')
    [[ -z "$DEFAULT_INTERNAL" ]] && DEFAULT_INTERNAL=$(hostname -I | awk '{print $1}')

    # Best-guess public IP (used only as a default when prompting).
    local DEFAULT_EXTERNAL="$DEFAULT_INTERNAL"
    local DETECTED_PUBLIC
    DETECTED_PUBLIC=$(curl -4 -s --connect-timeout 3 https://api.ipify.org 2>/dev/null || true)
    if [[ -n "$DETECTED_PUBLIC" && "$DETECTED_PUBLIC" != "$DEFAULT_INTERNAL" ]]; then
        DEFAULT_EXTERNAL="$DETECTED_PUBLIC"
    fi

    # --- Our signaling IPs -----------------------------------------------
    if [[ -z "$INTERNAL_SIP_IP" ]]; then
        if [[ "$NON_INTERACTIVE" == "true" ]]; then
            INTERNAL_SIP_IP="$DEFAULT_INTERNAL"
        else
            echo "Our LAN SIP IP (FreeSWITCH 5060/5062 + Kamailio FS-facing 5080)."
            read -r -p "  Internal SIP IP [$DEFAULT_INTERNAL]: " INTERNAL_SIP_IP
            INTERNAL_SIP_IP="${INTERNAL_SIP_IP:-$DEFAULT_INTERNAL}"
        fi
    fi
    if [[ -z "$EXTERNAL_SIP_IP" ]]; then
        if [[ "$NON_INTERACTIVE" == "true" ]]; then
            EXTERNAL_SIP_IP="$DEFAULT_EXTERNAL"
        else
            echo "Our public SIP IP (Kamailio :5060 carrier-facing listener)."
            read -r -p "  External SIP IP [$DEFAULT_EXTERNAL]: " EXTERNAL_SIP_IP
            EXTERNAL_SIP_IP="${EXTERNAL_SIP_IP:-$DEFAULT_EXTERNAL}"
        fi
    fi

    # --- Our media IPs (RTPEngine) ---------------------------------------
    if [[ -z "$INTERNAL_MEDIA_IP" ]]; then
        if [[ "$NON_INTERACTIVE" == "true" ]]; then
            INTERNAL_MEDIA_IP="$INTERNAL_SIP_IP"
        else
            echo "RTPEngine 'internal' media IP (usually the LAN IP)."
            read -r -p "  Internal Media IP [$INTERNAL_SIP_IP]: " INTERNAL_MEDIA_IP
            INTERNAL_MEDIA_IP="${INTERNAL_MEDIA_IP:-$INTERNAL_SIP_IP}"
        fi
    fi
    if [[ -z "$EXTERNAL_MEDIA_IP" ]]; then
        if [[ "$NON_INTERACTIVE" == "true" ]]; then
            EXTERNAL_MEDIA_IP="$EXTERNAL_SIP_IP"
        else
            echo "RTPEngine 'external' media IP (public media egress)."
            read -r -p "  External Media IP [$EXTERNAL_SIP_IP]: " EXTERNAL_MEDIA_IP
            EXTERNAL_MEDIA_IP="${EXTERNAL_MEDIA_IP:-$EXTERNAL_SIP_IP}"
        fi
    fi

    # --- Carrier (peer) IPs ----------------------------------------------
    if [[ -z "$CARRIER_SIP_IP" ]]; then
        if [[ "$NON_INTERACTIVE" == "true" ]]; then
            CARRIER_SIP_IP="$EXTERNAL_SIP_IP"
        else
            echo "Carrier's SIP IP (peer, used in Kamailio dr_gateways + ACL)."
            read -r -p "  Carrier SIP IP: " CARRIER_SIP_IP
        fi
    fi
    if [[ -z "$CARRIER_MEDIA_IP" ]]; then
        if [[ "$NON_INTERACTIVE" == "true" ]]; then
            CARRIER_MEDIA_IP="$CARRIER_SIP_IP"
        else
            echo "Carrier's RTP source IP (peer, added to media ACL)."
            read -r -p "  Carrier Media IP [$CARRIER_SIP_IP]: " CARRIER_MEDIA_IP
            CARRIER_MEDIA_IP="${CARRIER_MEDIA_IP:-$CARRIER_SIP_IP}"
        fi
    fi

    # Validate all six are IPv4 literals.
    local IPV4_RE='^([0-9]{1,3}\.){3}[0-9]{1,3}$'
    local ip
    for ip in INTERNAL_SIP_IP EXTERNAL_SIP_IP INTERNAL_MEDIA_IP \
              EXTERNAL_MEDIA_IP CARRIER_SIP_IP CARRIER_MEDIA_IP; do
        if [[ ! "${!ip}" =~ $IPV4_RE ]]; then
            log_error "$ip '${!ip}' is not a valid IPv4 address"
            exit 1
        fi
    done

    # Derive CIDR of the subnet INTERNAL_SIP_IP belongs to.
    INTERNAL_IP_NET=$(ip -4 route show | awk -v ip="$INTERNAL_SIP_IP" '$1 ~ /\// && index($0,"src "ip){print $1; exit}')
    [[ -z "$INTERNAL_IP_NET" ]] && INTERNAL_IP_NET="${INTERNAL_SIP_IP}/32"

    # Back-compat aliases consumed by dSIPRouter's installer (-netm 1) and by
    # the Kamailio-substdef / FreeSWITCH patch logic further down. These MUST
    # match the signaling-plane IPs, not the media ones.
    export INTERNAL_IP_ADDR="$INTERNAL_SIP_IP"
    export EXTERNAL_IP_ADDR="$EXTERNAL_SIP_IP"
    export INTERNAL_IP_NET
    export INTERNAL_SIP_IP EXTERNAL_SIP_IP INTERNAL_MEDIA_IP EXTERNAL_MEDIA_IP
    export CARRIER_SIP_IP CARRIER_MEDIA_IP
    export INTERNAL_FQDN="${INTERNAL_FQDN:-$(hostname -f 2>/dev/null || hostname)}"
    export EXTERNAL_FQDN="${EXTERNAL_FQDN:-$INTERNAL_FQDN}"
    export UAC_REG_ADDR="${UAC_REG_ADDR:-$EXTERNAL_SIP_IP}"

    log_info "Internal SIP IP:     $INTERNAL_SIP_IP   (subnet $INTERNAL_IP_NET)"
    log_info "External SIP IP:     $EXTERNAL_SIP_IP   (Kamailio :5060 carrier-face)"
    log_info "Internal Media IP:   $INTERNAL_MEDIA_IP (RTPEngine internal)"
    log_info "External Media IP:   $EXTERNAL_MEDIA_IP (RTPEngine external)"
    log_info "Carrier SIP IP:      $CARRIER_SIP_IP    (peer, ACL+dr_gateways)"
    log_info "Carrier Media IP:    $CARRIER_MEDIA_IP  (peer, RTP ACL)"
    log_success "Network configuration captured"
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

    # Create/sync role and database idempotently.
    # Re-runs must NOT fail if the role or DB already exist; instead we update
    # the password and reassign ownership so every subsequent step uses the
    # same credentials/owner as stored in /root/.apolo_credentials.
    sudo -u postgres psql -v ON_ERROR_STOP=1 <<EOF
DO \$\$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '$DB_USER') THEN
        CREATE ROLE $DB_USER LOGIN PASSWORD '$DB_PASSWORD';
    ELSE
        ALTER ROLE $DB_USER WITH LOGIN PASSWORD '$DB_PASSWORD';
    END IF;
END
\$\$;
EOF

    # Create DB only if missing; if it already exists, make sure it's owned
    # by our role so apply_database_schema can drop/recreate the public schema.
    DB_EXISTS=$(sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname='apolo_billing'")
    if [[ "$DB_EXISTS" != "1" ]]; then
        sudo -u postgres createdb -O "$DB_USER" apolo_billing
    else
        sudo -u postgres psql -v ON_ERROR_STOP=1 <<EOF
ALTER DATABASE apolo_billing OWNER TO $DB_USER;
EOF
    fi

    sudo -u postgres psql -v ON_ERROR_STOP=1 -d apolo_billing <<EOF
GRANT ALL PRIVILEGES ON DATABASE apolo_billing TO $DB_USER;
GRANT ALL ON SCHEMA public TO $DB_USER;
ALTER SCHEMA public OWNER TO $DB_USER;
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

    apt-get install -y mariadb-server mariadb-client libjemalloc2

    # Use jemalloc instead of glibc malloc: long-running mariadbd processes
    # under sustained temp-table churn fragment glibc's heap badly (observed
    # RSS of 4.87G against ~380M of configured buffers after 8.9 days uptime
    # on a production box). jemalloc avoids that growth.
    mkdir -p /etc/systemd/system/mariadb.service.d
    JEMALLOC_PATH=$(find /usr/lib -name 'libjemalloc.so.2' 2>/dev/null | head -1)
    cat > /etc/systemd/system/mariadb.service.d/override.conf <<EOF
[Service]
Environment="LD_PRELOAD=${JEMALLOC_PATH}"
EOF

    # Buffer pool / temp table tuning: the stock defaults (128M buffer pool,
    # 16M temp tables) are sized for a toy install, not a host with several
    # hundred MB of kamailio.acc/dr_* data and concurrent app connections —
    # they caused constant disk-backed temp tables and buffer pool eviction.
    cat > /etc/mysql/mariadb.conf.d/60-apolo-tuning.cnf <<'EOF'
[mysqld]
innodb_buffer_pool_size = 640M
tmp_table_size          = 64M
max_heap_table_size     = 64M
slow_query_log          = 1
long_query_time         = 2
EOF

    systemctl daemon-reload

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
# Kamailio acc table purge (retention cron)
#===============================================================================
configure_acc_purge() {
    log_section "Configuring kamailio.acc retention purge"

    local purge_script="$APOLO_DIR/scripts/purge_kamailio_acc.sh"
    if [[ ! -f "$purge_script" ]]; then
        log_warn "purge_kamailio_acc.sh not found in repo — skipping cron setup"
        return
    fi
    chmod +x "$purge_script"

    local cron_line="30 3 * * * $purge_script 90 >> /var/log/apolo-purge-acc.log 2>&1  # ApoloBilling: retencion 90d de kamailio.acc"
    ( crontab -l -u root 2>/dev/null | grep -vF "purge_kamailio_acc.sh"; echo "$cron_line" ) | crontab -u root -

    log_success "Daily purge of kamailio.acc (90d retention) scheduled at 03:30"
}

#===============================================================================
# sysstat (CPU/mem/net history for capacity planning)
#===============================================================================
configure_sysstat() {
    log_section "Configuring sysstat"

    apt-get install -y sysstat
    sed -i 's/^ENABLED=.*/ENABLED="true"/' /etc/default/sysstat
    sed -i 's/^HISTORY=.*/HISTORY=28/' /etc/sysstat/sysstat
    # 1-minute samples: the default 10 min hides short call peaks
    mkdir -p /etc/systemd/system/sysstat-collect.timer.d
    cat > /etc/systemd/system/sysstat-collect.timer.d/apolo-1min.conf <<EOF
[Timer]
OnCalendar=
OnCalendar=*:*:00
EOF
    systemctl daemon-reload
    systemctl enable --now sysstat sysstat-collect.timer sysstat-summary.timer

    log_success "sysstat collecting every minute (28 days history, see: sar -u)"
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

    # Clone dSIPRouter if not exists; otherwise reset to a clean state.
    # The dSIPRouter installer patches tracked files in place (kamailio.cfg,
    # settings.py, etc.), so `git pull` aborts on re-runs. Hard reset is safe
    # because those edits get re-applied by `./dsiprouter.sh install` below.
    if [[ ! -d /opt/dsiprouter ]]; then
        git clone https://github.com/dOpensource/dsiprouter.git
    else
        cd /opt/dsiprouter
        git fetch origin master
        git reset --hard origin/master
        git clean -fd
    fi

    cd /opt/dsiprouter

    # Install with RTPEngine (-all flag)
    # Use expect to handle interactive prompts, or run non-interactively
    export DEBIAN_FRONTEND=noninteractive

    # Run the installer.
    # The old `-servernat` flag was removed upstream. We use NETWORK_MODE=1
    # (-netm 1) and pre-export INTERNAL_IP_ADDR / EXTERNAL_IP_ADDR / INTERNAL_IP_NET
    # (set earlier by configure_network). When the two IPs differ, dSIPRouter
    # itself will turn on SIGNAL_SERVERNAT=1 and generate:
    #   - Kamailio:  listen = udp:INTERNAL:5060 advertise EXTERNAL:5060
    #   - RTPEngine: interface = ipv4/INTERNAL!EXTERNAL
    ./dsiprouter.sh install -all -netm 1

    # Wait for services to start
    sleep 10

    # Get dSIPRouter credentials from the installation
    if [[ -f /etc/dsiprouter/gui/settings.py ]]; then
        DSIP_PASSWORD=$(grep "DSIP_PASSWORD" /etc/dsiprouter/gui/settings.py 2>/dev/null | cut -d"'" -f2 || echo "admin")
        echo "DSIP_PASSWORD=$DSIP_PASSWORD" >> /root/.apolo_credentials
    fi
    {
        echo "INTERNAL_SIP_IP=$INTERNAL_SIP_IP"
        echo "EXTERNAL_SIP_IP=$EXTERNAL_SIP_IP"
        echo "INTERNAL_MEDIA_IP=$INTERNAL_MEDIA_IP"
        echo "EXTERNAL_MEDIA_IP=$EXTERNAL_MEDIA_IP"
        echo "CARRIER_SIP_IP=$CARRIER_SIP_IP"
        echo "CARRIER_MEDIA_IP=$CARRIER_MEDIA_IP"
    } >> /root/.apolo_credentials

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
# Detach Kamailio + RTPEngine from dSIPRouter control
#===============================================================================
# After install_dsiprouter runs, the dsiprouter systemd service owns
# /etc/kamailio/kamailio.cfg and /etc/rtpengine/rtpengine.conf — its Python
# backend rewrites them whenever an operator touches the GUI. We want those
# two services to run standalone with the configs Apolo deploys, so:
#   1. stop+disable dsiprouter.service
#   2. mask it so nothing re-enables it at boot
#   3. stop+disable dsiprouter-mysql.service if present (it's just a wrapper
#      around mariadb that expects dsiprouter's own lifecycle)
#   4. same for dsip-init.service, which runs `dsiprouter updatekamconfig` /
#      `updatedsipconfig` on every boot
#   5. move the `dsiprouter` CLI out of PATH so `configurekam`,
#      `generatekamconfig`, `configurertp`, `install` or an upgrade can't
#      replace /etc/kamailio/kamailio.cfg and /etc/rtpengine/rtpengine.conf
#      with symlinks to the stale copies under /etc/dsiprouter
#   6. replace the `dsiprouter renewsslcert` cron with kamailio-cert-renew,
#      since Kamailio still uses the TLS cert in /etc/dsiprouter/certs
# This is reversible: `systemctl unmask dsiprouter dsip-init && systemctl
# enable --now dsiprouter`, then `mv /usr/bin/dsiprouter.disabled
# /usr/bin/dsiprouter` restores the original behavior.
detach_from_dsiprouter() {
    log_section "Detaching Kamailio + RTPEngine from dSIPRouter"

    local svc
    for svc in dsiprouter dsiprouter-mysql dsip-init; do
        if systemctl list-unit-files "${svc}.service" &>/dev/null; then
            systemctl stop    "$svc" 2>/dev/null || true
            systemctl disable "$svc" 2>/dev/null || true
            systemctl mask    "$svc" 2>/dev/null || true
            log_info "  $svc: stopped, disabled, masked"
        fi
    done

    # Ensure Kamailio and RTPEngine are enabled independently so they come
    # up at boot without dsiprouter coordinating them.
    systemctl enable kamailio   2>/dev/null || true
    systemctl enable rtpengine  2>/dev/null || true

    # dsiprouter.sh install leaves these as symlinks into /etc/dsiprouter
    # (chattr +i'd by the stock units). deploy_apolo_configs' `cp -a` would
    # write through the link — or fail on the immutable target — so turn
    # them into regular files that Kamailio/RTPEngine own outright.
    local cfg target
    for cfg in /etc/kamailio/kamailio.cfg /etc/rtpengine/rtpengine.conf; do
        if [[ -L "$cfg" ]]; then
            target=$(readlink -f "$cfg")
            rm -f "$cfg"
            if [[ -f "$target" ]]; then
                cp -p "$target" "$cfg"
            fi
            log_info "  $cfg: symlink to $target replaced by a regular file"
        fi
    done

    if [[ -e /usr/bin/dsiprouter || -L /usr/bin/dsiprouter ]]; then
        mv -f /usr/bin/dsiprouter /usr/bin/dsiprouter.disabled
        hash -r
        log_info "  dsiprouter CLI moved to /usr/bin/dsiprouter.disabled"
    fi

    local cert_script="$APOLO_DIR/config/kamailio/kamailio-cert-renew.sh"
    if [[ -f "$cert_script" ]]; then
        install -m 755 "$cert_script" /usr/local/sbin/kamailio-cert-renew
        local cron_line="0 3 * * * /usr/local/sbin/kamailio-cert-renew"
        ( crontab -l -u root 2>/dev/null | grep -vF "dsiprouter renewsslcert" | grep -vF "kamailio-cert-renew"; echo "$cron_line" ) | crontab -u root -
        log_info "  dsiprouter renewsslcert cron replaced by kamailio-cert-renew"
    else
        log_warn "kamailio-cert-renew.sh not found in repo — TLS cert will not auto-renew"
    fi

    log_success "dSIPRouter detached; Kamailio + RTPEngine run standalone"
}

#===============================================================================
# Deploy ApoloBilling-curated VoIP configs on top of stock installs
#===============================================================================
# The base installers (fsget for FreeSWITCH, dsiprouter for Kamailio+RTPEngine)
# lay down generic defaults. This function overlays the hand-tuned configs we
# keep under /opt/ApoloBilling/config/ onto the live system paths, with .orig
# backups of whatever was there. Server-specific IPs in the Kamailio and
# RTPEngine files are rewritten from the INTERNAL_IP_ADDR / EXTERNAL_IP_ADDR
# captured by configure_network().
deploy_apolo_configs() {
    log_section "Deploying ApoloBilling configs"

    local SRC="$APOLO_DIR/config"
    if [[ ! -d "$SRC" ]]; then
        log_warn "$SRC not found — skipping config overlay"
        return 0
    fi

    # --- FreeSWITCH: overlay vars.xml, sip_profiles/, dialplan/, autoload_configs/ ---
    if [[ -d "$SRC/freeswitch" && -d /etc/freeswitch ]]; then
        log_info "Overlaying FreeSWITCH configs → /etc/freeswitch"
        local subdir
        for subdir in autoload_configs dialplan sip_profiles; do
            if [[ -d "$SRC/freeswitch/$subdir" ]]; then
                # Back up any pre-existing matching files (one-time snapshot).
                find "$SRC/freeswitch/$subdir" -type f | while read -r src_file; do
                    rel="${src_file#$SRC/freeswitch/$subdir/}"
                    dst="/etc/freeswitch/$subdir/$rel"
                    if [[ -f "$dst" && ! -f "$dst.orig" ]]; then
                        cp -a "$dst" "$dst.orig"
                    fi
                done
                cp -a "$SRC/freeswitch/$subdir/." "/etc/freeswitch/$subdir/"
            fi
        done
        if [[ -f "$SRC/freeswitch/vars.xml" ]]; then
            [[ -f /etc/freeswitch/vars.xml && ! -f /etc/freeswitch/vars.xml.orig ]] \
                && cp -a /etc/freeswitch/vars.xml /etc/freeswitch/vars.xml.orig
            cp -a "$SRC/freeswitch/vars.xml" /etc/freeswitch/vars.xml
        fi

        # Rewrite the repo's placeholder LAN IP (10.118.0.6) to this host's
        # actual IPs. Two passes so sip-ip and rtp-ip can diverge when the
        # media plane rides a different interface than signaling:
        #   1. `name="rtp-ip" value="..."` → $INTERNAL_MEDIA_IP
        #   2. every remaining 10.118.0.6  → $INTERNAL_SIP_IP
        # Scoped to the files we just dropped so stock FS configs untouched.
        local fs_files
        mapfile -t fs_files < <(grep -RIl "10\.118\.0\.6" \
            /etc/freeswitch/sip_profiles \
            /etc/freeswitch/dialplan \
            /etc/freeswitch/autoload_configs \
            2>/dev/null)
        if [[ ${#fs_files[@]} -gt 0 ]]; then
            if [[ "$INTERNAL_MEDIA_IP" != "10.118.0.6" ]]; then
                log_info "Patching FS rtp-ip: 10.118.0.6 → $INTERNAL_MEDIA_IP"
                sed -i -E \
                    "s|(name=\"rtp-ip\" value=\")10\.118\.0\.6(\")|\1${INTERNAL_MEDIA_IP}\2|g" \
                    "${fs_files[@]}"
            fi
            if [[ "$INTERNAL_SIP_IP" != "10.118.0.6" ]]; then
                log_info "Patching FS remaining 10.118.0.6 → $INTERNAL_SIP_IP"
                sed -i "s|10\.118\.0\.6|${INTERNAL_SIP_IP}|g" "${fs_files[@]}"
            fi
        fi

        chown -R freeswitch:freeswitch /etc/freeswitch

        # The apolo backend user rewrites /etc/freeswitch/dialplan/from-kamailio.xml
        # on every inbound-route create/update/delete. Give it group write +
        # the setgid bit on the dialplan directory so new files keep the
        # freeswitch group and are readable by the running daemon.
        if id apolo &>/dev/null && getent group freeswitch &>/dev/null; then
            chown -R apolo:freeswitch /etc/freeswitch/dialplan
            find /etc/freeswitch/dialplan -type d -exec chmod 2775 {} \;
            find /etc/freeswitch/dialplan -type f -exec chmod 664 {} \;

            # Same treatment for sip_profiles/external/gateways: the backend
            # writes one XML per outbound trunk under this directory when an
            # admin adds/edits a trunk in the UI (unified_routing handler).
            # Stock perms are freeswitch:freeswitch 755 which 403s the apolo
            # user with "Failed to write gateway file: Permission denied".
            if [[ -d /etc/freeswitch/sip_profiles/external/gateways ]]; then
                chown -R apolo:freeswitch /etc/freeswitch/sip_profiles/external/gateways
                find /etc/freeswitch/sip_profiles/external/gateways -type d -exec chmod 2775 {} \;
                find /etc/freeswitch/sip_profiles/external/gateways -type f -exec chmod 664 {} \;
            fi
        fi

        log_success "FreeSWITCH configs deployed (originals kept as .orig)"
    fi

    # --- Kamailio: replace kamailio.cfg, substituting IP + DB placeholders ---
    if [[ -f "$SRC/kamailio/kamailio.cfg" && -d /etc/kamailio ]]; then
        # If OpenSIPS was previously installed it grabs UDP/TCP 5060 on every
        # interface and Kamailio will fail to bind with "Address already in
        # use". This project's `sip_architecture=kamailio_freeswitch` mode
        # requires Kamailio on 5060, so stop and disable any lingering
        # OpenSIPS instance. Users on the OpenSIPS architecture should not be
        # running this installer.
        if systemctl list-unit-files opensips.service &>/dev/null; then
            systemctl stop opensips 2>/dev/null || true
            systemctl disable opensips 2>/dev/null || true
            log_info "Stopped/disabled opensips to free port 5060 for Kamailio"
        fi

        # dSIPRouter's installer may have rotated MySQL credentials created
        # earlier by install_mysql(). Re-assert our user and grants so the
        # cfg we are about to deploy (which authenticates as $DB_USER) works.
        mysql -u root <<EOF 2>/dev/null || true
CREATE USER IF NOT EXISTS '$DB_USER'@'localhost' IDENTIFIED BY '$DB_PASSWORD';
ALTER USER '$DB_USER'@'localhost' IDENTIFIED BY '$DB_PASSWORD';
GRANT ALL PRIVILEGES ON kamailio.* TO '$DB_USER'@'localhost';
FLUSH PRIVILEGES;
EOF
        log_info "Deploying kamailio.cfg → /etc/kamailio/kamailio.cfg"
        [[ -f /etc/kamailio/kamailio.cfg && ! -f /etc/kamailio/kamailio.cfg.orig ]] \
            && cp -a /etc/kamailio/kamailio.cfg /etc/kamailio/kamailio.cfg.orig
        cp -a "$SRC/kamailio/kamailio.cfg" /etc/kamailio/kamailio.cfg

        # Rewrite the #!substdef placeholders at the top of the file so they
        # reflect this server's network, not the repo author's LAN. We use
        # '|' as the sed delimiter because '!' appears literally in these
        # substdef lines (format: #!substdef "!NAME!VALUE!g").
        #
        # Semantics in Apolo's custom listen block:
        #   INTERNAL_IP_ADDR = our LAN SIP IP  (FS-facing listen on 5080)
        #   EXTERNAL_IP_ADDR = our public SIP IP (carrier-facing listen on 5060)
        # CARRIER_SIP_IP is the PEER's IP — it lives in dr_gateways/address,
        # not in Kamailio's listen substdefs.
        sed -i -E \
            -e "s|^(#!substdef \"!INTERNAL_IP_ADDR!)[^!]+(!g\")|\1${INTERNAL_SIP_IP}\2|" \
            -e "s|^(#!substdef \"!INTERNAL_IP_NET!)[^!]+(!g\")|\1${INTERNAL_IP_NET//\//\\/}\2|" \
            -e "s|^(#!substdef \"!EXTERNAL_IP_ADDR!)[^!]+(!g\")|\1${EXTERNAL_SIP_IP}\2|" \
            -e "s|^(#!substdef \"!UAC_REG_ADDR!)[^!]+(!g\")|\1${UAC_REG_ADDR}\2|" \
            /etc/kamailio/kamailio.cfg

        # Replace the MySQL credentials placeholder with our apolo user, which
        # already has GRANT ALL on the kamailio schema (see configure_kamailio).
        # We avoid touching the `kamailio` MySQL user because dSIPRouter owns
        # its password (stored encrypted in /etc/dsiprouter/gui/settings.py),
        # and rotating it would break the dSIPRouter GUI.
        sed -i \
            -e "s|mysql://kamailio:YOUR_KAMAILIO_DB_PASSWORD@|mysql://${DB_USER}:${DB_PASSWORD}@|g" \
            /etc/kamailio/kamailio.cfg

        # The repo cfg ships with WITH_DMZ and WITH_SIGNAL_SERVERNAT disabled,
        # replaced by Apolo's custom listen block (FS on INTERNAL:5080 +
        # carrier on EXTERNAL:5060). No further edits needed for listeners.
        chown kamailio:kamailio /etc/kamailio/kamailio.cfg 2>/dev/null || true
        log_success "kamailio.cfg deployed (IPs + MySQL creds patched)"
    fi

    # --- Kamailio: raise open-file limit for the children=8 worker pool ---
    # A drop-in override (not editing the packaged unit) so it survives a
    # `kamailio` package upgrade. The system default (1024 soft) is easily
    # exhausted by TCP/TLS/WSS connections across 8 parallel workers.
    if [[ -f "$SRC/kamailio/systemd/override.conf" ]]; then
        mkdir -p /etc/systemd/system/kamailio.service.d
        cp -a "$SRC/kamailio/systemd/override.conf" /etc/systemd/system/kamailio.service.d/override.conf
        systemctl daemon-reload
        log_success "kamailio.service.d/override.conf deployed (LimitNOFILE=65536)"
    fi

    # --- RTPEngine: replace interface line with our IPs ---
    if [[ -f "$SRC/rtpengine/rtpengine.conf" && -d /etc/rtpengine ]]; then
        log_info "Deploying rtpengine.conf → /etc/rtpengine/rtpengine.conf"
        [[ -f /etc/rtpengine/rtpengine.conf && ! -f /etc/rtpengine/rtpengine.conf.orig ]] \
            && cp -a /etc/rtpengine/rtpengine.conf /etc/rtpengine/rtpengine.conf.orig
        cp -a "$SRC/rtpengine/rtpengine.conf" /etc/rtpengine/rtpengine.conf
        # Media-plane interfaces: the INTERNAL_MEDIA_IP can be the same as
        # INTERNAL_SIP_IP (most deployments) or a dedicated media NIC;
        # EXTERNAL_MEDIA_IP is the public egress — often differs from the
        # signaling public IP when media rides its own address.
        sed -i -E \
            "s|^interface[[:space:]]*=.*|interface = internal/${INTERNAL_MEDIA_IP};external/${EXTERNAL_MEDIA_IP}|" \
            /etc/rtpengine/rtpengine.conf
        log_success "rtpengine.conf deployed (interface=internal/$INTERNAL_MEDIA_IP;external/$EXTERNAL_MEDIA_IP)"
    fi

    # --- RTPEngine: drop dSIPRouter leftovers from the packaged unit ---
    # The stock rtpengine.service does chattr -i/+i on
    # /etc/dsiprouter/rtpengine/rtpengine.conf plus `dsiprouter chown`
    # on every start/stop. RTPEngine runs standalone here with
    # CONFIG_FILE=/etc/rtpengine/rtpengine.conf, so those lines only touch
    # an orphan file. Drop-in (not editing /lib) so package upgrades keep it.
    if [[ -f "$SRC/rtpengine/systemd/override.conf" ]]; then
        mkdir -p /etc/systemd/system/rtpengine.service.d
        cp -a "$SRC/rtpengine/systemd/override.conf" /etc/systemd/system/rtpengine.service.d/override.conf
        systemctl daemon-reload
        log_success "rtpengine.service.d/override.conf deployed (dsiprouter chattr/chown removed)"
    fi
    if [[ -f "$SRC/rtpengine/rtpengine-kernel-check.sh" ]]; then
        cp -a "$SRC/rtpengine/rtpengine-kernel-check.sh" /usr/local/sbin/rtpengine-kernel-check
        chmod 755 /usr/local/sbin/rtpengine-kernel-check
        log_success "rtpengine-kernel-check installed (post-start kernel self-check)"
    fi

    # --- RTPEngine kernel mode needs the iptables-legacy backend ---
    # xt_RTPENGINE is an xtables extension: only the legacy backend can
    # create/inspect its `-j RTPENGINE` rule, while ngcp-rtpengine-iptables-setup
    # calls the generic `iptables`/`ip6tables` CLIs and hides all errors
    # (2>/dev/null). Without this pin the kernel-forwarding rule may silently
    # miss at startup while the daemon falls back to userspace. nftables
    # rulesets (firewalld) are untouched by this switch; docker shells out to
    # the same CLIs, so restart it to keep its backend detection consistent.
    for alt in iptables ip6tables; do
        update-alternatives --set $alt /usr/sbin/${alt}-legacy 2>/dev/null || true
    done
    if systemctl is-active --quiet docker 2>/dev/null; then
        systemctl restart docker 2>/dev/null || true
        log_info "docker restarted (iptables backend consistency)"
    fi
    log_success "iptables/ip6tables pinned to legacy (rtpengine kernel requirement)"

    # Restart so the new configs take effect (ignore failures: configure_*
    # steps below may still tweak things and restart again).
    systemctl restart rtpengine 2>/dev/null || true
    systemctl restart kamailio  2>/dev/null || true
    systemctl restart freeswitch 2>/dev/null || true
}

#===============================================================================
# Configure FreeSWITCH for ApoloBilling
#===============================================================================
configure_freeswitch() {
    log_section "Configuring FreeSWITCH for ApoloBilling"

    # All profile/dialplan/xml_curl/kamailio-gateway edits now live in the
    # repo's config/freeswitch/ tree, deployed by deploy_apolo_configs().
    # This function exists only to apply small post-deploy tweaks and
    # restart the service.
    systemctl restart freeswitch 2>/dev/null || true
    log_success "FreeSWITCH restarted"
}

#===============================================================================
# Configure Kamailio for FreeSWITCH
#===============================================================================
configure_kamailio() {
    log_section "Configuring Kamailio for FreeSWITCH"

    SERVER_IP=$(hostname -I | awk '{print $1}')

    # dSIPRouter's installer rotates MySQL credentials (creates its own
    # `kamailio` user and resets previously created accounts). Re-apply the
    # apolo user's password and grants so the INSERTs below work on every run.
    mysql -u root <<EOF
CREATE USER IF NOT EXISTS '$DB_USER'@'localhost' IDENTIFIED BY '$DB_PASSWORD';
ALTER USER '$DB_USER'@'localhost' IDENTIFIED BY '$DB_PASSWORD';
GRANT ALL PRIVILEGES ON kamailio.* TO '$DB_USER'@'localhost';
FLUSH PRIVILEGES;
EOF

    # Register FreeSWITCH and the PSTN carrier as known peers in Kamailio's
    # MySQL. FreeSWITCH is type=9 reachable on our LAN IP port 5062 (its
    # external profile). The carrier is type=8 and we trust both its SIP
    # source (CARRIER_SIP_IP) and its RTP source (CARRIER_MEDIA_IP) via the
    # `address` table.
    mysql -u "$DB_USER" -p"$DB_PASSWORD" kamailio <<EOF
-- FreeSWITCH (LAN peer, receives carrier inbound). dSIPRouter's kamailio.cfg
-- expects \`dr_gateways.address\` as bare host:port — it prepends sip: itself
-- when building the next-hop RURI. An explicit sip: here yields sip:sip:...
-- and the RURI becomes unparseable (Kamailio replies 479).
INSERT INTO dr_gateways (gwid, type, address, strip, pri_prefix, attrs, description)
VALUES (100, 9, '${INTERNAL_SIP_IP}:5062', 0, '', '', 'FreeSWITCH external profile')
ON DUPLICATE KEY UPDATE address='${INTERNAL_SIP_IP}:5062',
                        description='FreeSWITCH external profile';

-- PSTN carrier
INSERT INTO dr_gateways (gwid, type, address, strip, pri_prefix, attrs, description)
VALUES (8, 8, '${CARRIER_SIP_IP}:5060', 0, '', '', 'PSTN carrier')
ON DUPLICATE KEY UPDATE address='${CARRIER_SIP_IP}:5060',
                        description='PSTN carrier';

-- Trust FS (LAN) in group 1; trust carrier SIP + RTP sources in group 2.
-- address has no unique key, so ON DUPLICATE KEY never fired and every
-- re-run added copies; insert only when the row is missing.
INSERT INTO address (grp, ip_addr, mask, port, tag)
SELECT 1, '${INTERNAL_SIP_IP}', 32, 5062, 'FreeSWITCH' FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM address WHERE grp = 1 AND ip_addr = '${INTERNAL_SIP_IP}' AND port = 5062);

INSERT INTO address (grp, ip_addr, mask, port, tag)
SELECT 2, '${CARRIER_SIP_IP}', 32, 5060, 'Carrier-SIP' FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM address WHERE grp = 2 AND ip_addr = '${CARRIER_SIP_IP}' AND port = 5060);

INSERT INTO address (grp, ip_addr, mask, port, tag)
SELECT 2, '${CARRIER_MEDIA_IP}', 32, 0, 'Carrier-RTP' FROM DUAL
WHERE NOT EXISTS (SELECT 1 FROM address WHERE grp = 2 AND ip_addr = '${CARRIER_MEDIA_IP}' AND port = 0);
EOF

    # Apolo schema additions on the Kamailio DB (idempotent, MariaDB 10.5+)
    local f
    for f in $(ls database/kamailio/*.sql 2>/dev/null | sort); do
        log_info "Applying Kamailio migration: $(basename "$f")"
        mysql -u "$DB_USER" -p"$DB_PASSWORD" kamailio < "$f" 2>&1 | sed 's/^/    /'
    done

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

    # Generate secrets. JWT_SECRET signs auth tokens. SIP_ENCRYPTION_KEY is the
    # AES-256-GCM key used to encrypt SIP device passwords at rest — if it is
    # missing, SipDeviceService::new() returns None and every POST /sip-devices
    # fails with HTTP 500 ("Requested application data is not configured
    # correctly") because the web::Data<Arc<SipDeviceService>> extractor can't
    # find the type. 32 hex bytes = 64 chars = 256 bits.
    JWT_SECRET=$(openssl rand -base64 32)
    SIP_ENCRYPTION_KEY=$(openssl rand -hex 32)

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

# SIP device password encryption (AES-256-GCM). Required for /sip-devices
# endpoints; generate with: openssl rand -hex 32
SIP_ENCRYPTION_KEY=${SIP_ENCRYPTION_KEY}

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

# Kamailio dialog CDRs for inbound calls routed direct (no FreeSWITCH)
KAMAILIO_DATABASE_URL=mysql://${DB_USER}:${DB_PASSWORD}@localhost:3306/kamailio

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
# schema.sql is a pg_dump snapshot: it has no IF NOT EXISTS guards and no
# ownership annotations, so rerunning it against a populated database fails
# with "relation already exists" / "must be owner of ..." errors.
#
# Strategy for an idempotent installer:
#   1. Detect whether the `public` schema already contains ApoloBilling tables.
#   2. If so, refuse to destroy data unless --reset-db was passed (or the caller
#      confirms interactively). Treat RESET_DB=true from the environment as
#      equivalent.
#   3. When resetting, DROP SCHEMA public CASCADE and recreate it as $DB_USER
#      so schema.sql runs against an empty schema.
#   4. Run schema.sql as the postgres superuser — this bypasses any stale
#      ownership from previous runs and is safe because we immediately REASSIGN
#      every public-schema object to $DB_USER afterwards.
#   5. Grant default privileges so future objects stay usable by $DB_USER.
apply_database_schema() {
    log_section "Applying Database Schema"

    cd "$APOLO_DIR"

    if [[ ! -f database/schema.sql ]]; then
        log_warn "database/schema.sql not found, skipping schema load"
        return 0
    fi

    # Count ApoloBilling-owned tables (i.e. is this a populated DB already?)
    local EXISTING_TABLES
    EXISTING_TABLES=$(sudo -u postgres psql -tAc \
        "SELECT count(*) FROM information_schema.tables \
         WHERE table_schema='public' AND table_type='BASE TABLE'" \
        apolo_billing 2>/dev/null | tr -d ' ')
    EXISTING_TABLES=${EXISTING_TABLES:-0}

    if [[ "$EXISTING_TABLES" -gt 0 ]]; then
        log_warn "Database apolo_billing already contains $EXISTING_TABLES tables."
        if [[ "$RESET_DB" != "true" ]]; then
            if [[ "$NON_INTERACTIVE" == "true" ]]; then
                log_warn "Non-interactive mode: keeping existing schema (pass --reset-db to wipe)."
                log_warn "Skipping schema.sql load — migrations will still be applied."
                apply_migrations
                reassign_schema_ownership
                return 0
            fi
            echo ""
            echo -e "${YELLOW}WARNING:${NC} The public schema of apolo_billing is not empty."
            echo "Reapplying schema.sql requires wiping it (all data will be LOST)."
            read -r -p "Type 'RESET' to continue, anything else to keep data: " CONFIRM
            if [[ "$CONFIRM" != "RESET" ]]; then
                log_warn "Keeping existing schema. Migrations will still be applied."
                apply_migrations
                reassign_schema_ownership
                return 0
            fi
        else
            log_warn "--reset-db provided: wiping public schema."
        fi
    fi

    # Wipe and recreate the public schema so schema.sql starts from a clean slate.
    log_info "Resetting public schema (owner=$DB_USER)..."
    sudo -u postgres psql -v ON_ERROR_STOP=1 -d apolo_billing <<EOF
DROP SCHEMA IF EXISTS public CASCADE;
CREATE SCHEMA public AUTHORIZATION $DB_USER;
GRANT ALL ON SCHEMA public TO $DB_USER;
GRANT USAGE ON SCHEMA public TO PUBLIC;
EOF

    # schema.sql is a pg_dump that references trigger functions
    # (update_routing_updated_at, update_sip_devices_updated_at) but ships
    # WITHOUT their CREATE FUNCTION statements — those live only in migrations
    # 003/004, which run after schema.sql. Predeclare the functions as
    # NOW()-returning stubs so the CREATE TRIGGER lines inside schema.sql
    # resolve; the migrations' CREATE OR REPLACE will later install the final
    # identical bodies. Keep these definitions in lockstep with the
    # migrations' function bodies.
    log_info "Predeclaring trigger function stubs..."
    sudo -u postgres psql -v ON_ERROR_STOP=1 -d apolo_billing <<'EOF'
CREATE OR REPLACE FUNCTION public.update_routing_updated_at()
RETURNS TRIGGER AS $BODY$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$BODY$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION public.update_sip_devices_updated_at()
RETURNS TRIGGER AS $BODY$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$BODY$ LANGUAGE plpgsql;
EOF

    # Run the dump as superuser so we don't hit ownership errors on objects
    # that schema.sql tries to create without an explicit OWNER clause.
    log_info "Loading schema.sql as postgres superuser..."
    sudo -u postgres psql -v ON_ERROR_STOP=1 -d apolo_billing -f database/schema.sql
    log_success "Main schema applied"

    reassign_schema_ownership

    # Apply migrations even after a fresh schema.sql load: the dump is
    # incomplete (e.g. it omits `routing_sync_log` that migration 004 creates,
    # which the backend's sqlx compile-time queries need). Collisions with
    # objects already present in schema.sql are absorbed by ON_ERROR_STOP=0
    # inside apply_migrations.
    apply_migrations
}

# Transfer ownership of every object in the `public` schema to $DB_USER and
# (re)grant all privileges. Safe to call repeatedly.
#
# Why not `REASSIGN OWNED BY postgres TO $DB_USER`? Because `postgres` also
# owns cluster-wide system objects (extensions, catalogs, system functions)
# and PostgreSQL refuses with "no se puede reasignar la propiedad de objetos
# de rol postgres porque son requeridos por el sistema". We must scope the
# transfer to the `public` schema by iterating the catalogs ourselves.
reassign_schema_ownership() {
    log_info "Reassigning public-schema objects to $DB_USER..."
    sudo -u postgres psql -v ON_ERROR_STOP=1 -d apolo_billing <<EOF
DO \$\$
DECLARE
    obj RECORD;
BEGIN
    -- Tables / partitioned tables / foreign tables FIRST. Moving a table's
    -- owner automatically moves any sequence "owned by" one of its columns,
    -- so this single pass handles serial/identity sequences correctly.
    FOR obj IN
        SELECT c.relname, c.relkind
          FROM pg_class c
          JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = 'public'
           AND c.relkind IN ('r','f','p')
    LOOP
        EXECUTE format('ALTER TABLE public.%I OWNER TO %I',
                       obj.relname, '$DB_USER');
    END LOOP;

    -- Views / materialized views
    FOR obj IN
        SELECT c.relname, c.relkind
          FROM pg_class c
          JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = 'public'
           AND c.relkind IN ('v','m')
    LOOP
        IF obj.relkind = 'v' THEN
            EXECUTE format('ALTER VIEW public.%I OWNER TO %I',
                           obj.relname, '$DB_USER');
        ELSE
            EXECUTE format('ALTER MATERIALIZED VIEW public.%I OWNER TO %I',
                           obj.relname, '$DB_USER');
        END IF;
    END LOOP;

    -- Standalone sequences only. Sequences that are "owned by" a table
    -- column (pg_depend.deptype='a') cannot have their owner changed
    -- directly — PostgreSQL requires them to follow their parent table,
    -- and that transfer already happened in the loop above.
    FOR obj IN
        SELECT c.relname
          FROM pg_class c
          JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = 'public'
           AND c.relkind = 'S'
           AND NOT EXISTS (
               SELECT 1
                 FROM pg_depend d
                WHERE d.objid = c.oid
                  AND d.classid  = 'pg_class'::regclass
                  AND d.refclassid = 'pg_class'::regclass
                  AND d.deptype  = 'a'
           )
    LOOP
        EXECUTE format('ALTER SEQUENCE public.%I OWNER TO %I',
                       obj.relname, '$DB_USER');
    END LOOP;

    -- Functions and procedures
    FOR obj IN
        SELECT p.oid::regprocedure AS signature
          FROM pg_proc p
          JOIN pg_namespace n ON n.oid = p.pronamespace
         WHERE n.nspname = 'public'
    LOOP
        EXECUTE format('ALTER FUNCTION %s OWNER TO %I',
                       obj.signature, '$DB_USER');
    END LOOP;

    -- User-defined types / domains (not the implicit row types of tables)
    FOR obj IN
        SELECT t.typname
          FROM pg_type t
          JOIN pg_namespace n ON n.oid = t.typnamespace
         WHERE n.nspname = 'public'
           AND t.typtype IN ('c','d','e','r')
           AND NOT EXISTS (
               SELECT 1 FROM pg_class c
                WHERE c.reltype = t.oid OR c.reloftype = t.oid
           )
    LOOP
        EXECUTE format('ALTER TYPE public.%I OWNER TO %I',
                       obj.typname, '$DB_USER');
    END LOOP;
END
\$\$;

ALTER SCHEMA public OWNER TO $DB_USER;
GRANT ALL PRIVILEGES ON ALL TABLES    IN SCHEMA public TO $DB_USER;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO $DB_USER;
GRANT ALL PRIVILEGES ON ALL FUNCTIONS IN SCHEMA public TO $DB_USER;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON TABLES    TO $DB_USER;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON SEQUENCES TO $DB_USER;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON FUNCTIONS TO $DB_USER;
EOF
}

apply_migrations() {
    if [[ ! -d database/migrations ]]; then
        return 0
    fi
    local f
    # Migrations here are only invoked against a PRE-EXISTING database (the
    # "keep data" branch). We tolerate per-statement failures (ON_ERROR_STOP=0)
    # because not every migration is idempotent and most will re-create objects
    # already present in schema.sql. Genuine failures surface as psql errors
    # in the log; this tolerance is the price of not dropping user data.
    for f in $(ls database/migrations/*.sql 2>/dev/null | sort); do
        [[ -f "$f" ]] || continue
        log_info "Applying migration: $(basename "$f")"
        sudo -u postgres psql -v ON_ERROR_STOP=0 -d apolo_billing -f "$f" \
            2>&1 | sed 's/^/    /'
    done
    log_success "Migrations attempted (errors above are expected when re-running)"
    # Newly-created tables from migrations should also be owned by $DB_USER.
    reassign_schema_ownership
}

#===============================================================================
# Build ApoloBilling
#===============================================================================
build_apolo() {
    log_section "Building ApoloBilling"

    source "$HOME/.cargo/env"

    # Build Rust Backend + the gen_hash helper (used by create_admin_user
    # to produce a valid Argon2id hash without depending on python-argon2).
    log_info "Building Rust backend..."
    cd "$APOLO_DIR/rust-backend"
    cargo build --release
    cargo build --release --example gen_hash -p apolo-auth

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

    # Use the backend's own Argon2 implementation via the `gen_hash` example
    # binary (compiled in build_apolo). This guarantees the hash format is
    # exactly what apolo-auth verifies at login time, and avoids a dependency
    # on python3-argon2-cffi which the previous implementation installed via
    # pip and silently failed on PEP-668-managed systems, leaving the admin
    # user with a literal placeholder ("NEEDS_RESET:admin123") that could
    # never authenticate.
    local GEN_HASH="$APOLO_DIR/rust-backend/target/release/examples/gen_hash"
    local HASH=""

    if [[ -x "$GEN_HASH" ]]; then
        HASH=$("$GEN_HASH" "$ADMIN_PASSWORD" 2>/dev/null | awk '/^Hash:/ {print $2; exit}')
    fi

    if [[ -z "$HASH" || "$HASH" != \$argon2* ]]; then
        log_error "gen_hash did not produce a valid Argon2 hash — aborting admin creation."
        log_error "Rebuild with: cargo build --release --example gen_hash -p apolo-auth"
        return 1
    fi

    # Column name is `password` in the DB; the backend aliases it as
    # `password_hash` in its SELECTs (see crates/apolo-db user_repo.rs).
    PGPASSWORD="$DB_PASSWORD" psql -U "$DB_USER" -h localhost -d apolo_billing \
        -v ON_ERROR_STOP=1 -v hash="$HASH" -v user="$ADMIN_USER" <<'EOF'
INSERT INTO usuarios (username, password, nombre, apellido, email, role, activo)
VALUES (:'user', :'hash', 'Administrador', 'Sistema', 'admin@localhost', 'superadmin', true)
ON CONFLICT (username) DO UPDATE SET password = EXCLUDED.password, activo = true;
EOF

    log_success "Admin user '$ADMIN_USER' ready (password: $ADMIN_PASSWORD)"
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

    # Ask for internal/external IP up-front so the VoIP installers don't
    # block mid-run waiting for input (dSIPRouter takes 10-20 minutes).
    if [[ "$SKIP_VOIP" == "false" ]]; then
        configure_network
    fi

    update_system
    install_postgresql
    install_mysql
    install_redis
    install_nginx
    install_nodejs
    install_rust

    # Clone the repo before the VoIP layout step so deploy_apolo_configs can
    # read /opt/ApoloBilling/config/.
    create_apolo_user
    clone_repository
    configure_acc_purge
    configure_sysstat

    if [[ "$SKIP_VOIP" == "false" ]]; then
        install_dsiprouter
        install_freeswitch
        # Detach BEFORE deploy so dsiprouter can't race us and overwrite.
        detach_from_dsiprouter
        deploy_apolo_configs
        configure_freeswitch
        configure_kamailio
    else
        log_warn "Skipping VoIP components (--skip-voip flag)"
    fi
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
