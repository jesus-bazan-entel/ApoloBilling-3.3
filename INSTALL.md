# Guía Completa de Instalación - ApoloBilling + VoIP Stack

## Servidor Debian 12 Fresh Install

---

## Índice

1. [Requisitos y Preparación](#1-requisitos-y-preparación)
2. [PostgreSQL (ApoloBilling)](#2-postgresql-apolobilling)
3. [Redis](#3-redis)
4. [MySQL/MariaDB (Kamailio)](#4-mysqlmariadb-kamailio)
5. [Kamailio + dSIPRouter](#5-kamailio--dsiprouter)
6. [FreeSWITCH](#6-freeswitch)
7. [RTPEngine](#7-rtpengine)
8. [Rust Backend](#8-rust-backend)
9. [Rust Billing Engine](#9-rust-billing-engine)
10. [Frontend React](#10-frontend-react)
11. [Nginx](#11-nginx)
12. [Servicios Systemd](#12-servicios-systemd)
13. [Integración Final](#13-integración-final)
14. [Sistema de Routing Unificado](#14-sistema-de-routing-unificado)
15. [Dispositivos SIP y Monitoreo](#15-dispositivos-sip-y-monitoreo)
16. [Rutas Internas](#16-rutas-internas)
17. [Verificación](#17-verificación)

---

## 1. Requisitos y Preparación

### Hardware Mínimo

| Recurso | Mínimo | Recomendado |
|---------|--------|-------------|
| CPU | 4 cores | 8+ cores |
| RAM | 8 GB | 16+ GB |
| Disco | 50 GB SSD | 100+ GB SSD |
| Red | 100 Mbps | 1 Gbps |

### Puertos Requeridos

| Puerto | Servicio | Protocolo |
|--------|----------|-----------|
| 22 | SSH | TCP |
| 80/443 | Nginx (Web) | TCP |
| 3000 | Frontend (interno) | TCP |
| 5060 | Kamailio SIP | UDP/TCP |
| 5080 | FreeSWITCH Internal | UDP/TCP |
| 5062 | FreeSWITCH External | UDP/TCP |
| 8000 | API Backend | TCP |
| 8021 | FreeSWITCH ESL | TCP |
| 10000-20000 | RTP Media | UDP |

### 1.1 Actualizar sistema

```bash
apt update && apt upgrade -y
reboot
```

### 1.2 Instalar herramientas base

```bash
apt install -y \
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
    jq
```

### 1.3 Configurar hostname y timezone

```bash
hostnamectl set-hostname voip-server
timedatectl set-timezone America/Lima  # Ajustar según ubicación

# Agregar al /etc/hosts
echo "127.0.0.1 voip-server" >> /etc/hosts
```

---

## 2. PostgreSQL (ApoloBilling)

### 2.1 Instalar PostgreSQL 15

```bash
# Agregar repositorio oficial
sh -c 'echo "deb https://apt.postgresql.org/pub/repos/apt $(lsb_release -cs)-pgdg main" > /etc/apt/sources.list.d/pgdg.list'
wget --quiet -O - https://www.postgresql.org/media/keys/ACCC4CF8.asc | apt-key add -
apt update

apt install -y postgresql-15 postgresql-contrib-15

systemctl enable postgresql
systemctl start postgresql
```

### 2.2 Crear base de datos ApoloBilling

```bash
sudo -u postgres psql << 'EOF'
-- Crear usuario
CREATE USER apolo_user WITH PASSWORD 'TU_PASSWORD_SEGURO_PG';

-- Crear base de datos
CREATE DATABASE apolo_billing OWNER apolo_user;

-- Permisos
GRANT ALL PRIVILEGES ON DATABASE apolo_billing TO apolo_user;
\c apolo_billing
GRANT ALL ON SCHEMA public TO apolo_user;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON TABLES TO apolo_user;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT ALL ON SEQUENCES TO apolo_user;
\q
EOF
```

### 2.3 Configurar acceso remoto (opcional)

```bash
# Editar postgresql.conf
sed -i "s/#listen_addresses = 'localhost'/listen_addresses = '*'/" /etc/postgresql/15/main/postgresql.conf

# Editar pg_hba.conf (agregar al final)
echo "host    apolo_billing    apolo_user    0.0.0.0/0    scram-sha-256" >> /etc/postgresql/15/main/pg_hba.conf

systemctl restart postgresql
```

---

## 3. Redis

```bash
apt install -y redis-server

# Configurar
cat >> /etc/redis/redis.conf << 'EOF'
maxmemory 512mb
maxmemory-policy allkeys-lru
EOF

systemctl enable redis-server
systemctl restart redis-server

# Verificar
redis-cli ping  # Debe responder PONG
```

---

## 4. MySQL/MariaDB (Kamailio)

### 4.1 Instalar MariaDB

```bash
apt install -y mariadb-server mariadb-client

systemctl enable mariadb
systemctl start mariadb

# Asegurar instalación
mysql_secure_installation
# Responder: Y, [password], Y, Y, Y, Y
```

### 4.2 Crear base de datos para Kamailio

```bash
mysql -u root -p << 'EOF'
CREATE DATABASE kamailio;
CREATE USER 'kamailio'@'localhost' IDENTIFIED BY 'TU_PASSWORD_MYSQL';
CREATE USER 'kamailio'@'%' IDENTIFIED BY 'TU_PASSWORD_MYSQL';
GRANT ALL PRIVILEGES ON kamailio.* TO 'kamailio'@'localhost';
GRANT ALL PRIVILEGES ON kamailio.* TO 'kamailio'@'%';
FLUSH PRIVILEGES;
EOF
```

---

## 5. Kamailio + dSIPRouter

### 5.1 Instalar dependencias

```bash
apt install -y \
    python3 \
    python3-pip \
    python3-venv \
    python3-dev \
    default-libmysqlclient-dev \
    libmariadb-dev \
    libpq-dev \
    libcurl4-openssl-dev \
    libxml2-dev \
    libpcre3-dev
```

### 5.2 Instalar dSIPRouter (incluye Kamailio)

```bash
cd /opt

# Clonar dSIPRouter
git clone https://github.com/dOpensource/dsiprouter.git
cd dsiprouter

# Instalar (esto instala Kamailio automáticamente)
./dsiprouter.sh install -all -kam -dsip -rtp

# Durante la instalación se pedirá:
# - Password para dSIPRouter GUI
# - Password para Kamailio DB (usar el mismo que creaste)
```

### 5.3 Configurar Kamailio para ApoloBilling

```bash
# El archivo principal de configuración está en:
# /etc/kamailio/kamailio.cfg

# Verificar que ESL está configurado (para FreeSWITCH)
# y que drouting está habilitado
```

### 5.4 Iniciar servicios

```bash
systemctl enable kamailio
systemctl start kamailio

# Verificar
kamcmd dispatcher.list
kamcmd drouting.list
```

---

## 6. FreeSWITCH

### 6.1 Agregar repositorio SignalWire

```bash
# Token de SignalWire (obtener en signalwire.com)
TOKEN="TU_SIGNALWIRE_TOKEN"

apt install -y gnupg2 wget lsb-release

wget --http-user=signalwire --http-password=$TOKEN \
    -O /usr/share/keyrings/signalwire-freeswitch-repo.gpg \
    https://freeswitch.signalwire.com/repo/deb/debian-release/signalwire-freeswitch-repo.gpg

echo "machine freeswitch.signalwire.com login signalwire password $TOKEN" > /etc/apt/auth.conf
chmod 600 /etc/apt/auth.conf

echo "deb [signed-by=/usr/share/keyrings/signalwire-freeswitch-repo.gpg] https://freeswitch.signalwire.com/repo/deb/debian-release/ $(lsb_release -sc) main" \
    > /etc/apt/sources.list.d/freeswitch.list

apt update
```

### 6.2 Instalar FreeSWITCH

```bash
apt install -y freeswitch-meta-all

systemctl enable freeswitch
```

### 6.3 Configurar ESL (Event Socket Layer)

```bash
cat > /etc/freeswitch/autoload_configs/event_socket.conf.xml << 'EOF'
<configuration name="event_socket.conf" description="Socket Client">
  <settings>
    <param name="nat-map" value="false"/>
    <param name="listen-ip" value="0.0.0.0"/>
    <param name="listen-port" value="8021"/>
    <param name="password" value="ClueCon"/>
    <param name="apply-inbound-acl" value="loopback.auto"/>
  </settings>
</configuration>
EOF
```

### 6.4 Configurar SIP Profiles

```bash
# Internal Profile (para dispositivos registrados) - puerto 5080
cat > /etc/freeswitch/sip_profiles/internal.xml << 'EOF'
<profile name="internal">
  <settings>
    <param name="sip-ip" value="$${local_ip_v4}"/>
    <param name="sip-port" value="5080"/>
    <param name="rtp-ip" value="$${local_ip_v4}"/>
    <param name="context" value="from-pbx"/>
    <param name="dialplan" value="XML"/>
    <param name="auth-calls" value="true"/>
    <param name="apply-inbound-acl" value="domains"/>
    <param name="local-network-acl" value="localnet.auto"/>
  </settings>
</profile>
EOF

# External Profile (para troncales) - puerto 5062
cat > /etc/freeswitch/sip_profiles/external.xml << 'EOF'
<profile name="external">
  <settings>
    <param name="sip-ip" value="$${local_ip_v4}"/>
    <param name="sip-port" value="5062"/>
    <param name="rtp-ip" value="$${local_ip_v4}"/>
    <param name="context" value="public"/>
    <param name="dialplan" value="XML"/>
    <param name="auth-calls" value="false"/>
  </settings>
</profile>
EOF
```

### 6.5 Configurar mod_xml_curl (autenticación dinámica)

```bash
cat > /etc/freeswitch/autoload_configs/xml_curl.conf.xml << 'EOF'
<configuration name="xml_curl.conf" description="cURL XML Gateway">
  <bindings>
    <binding name="directory">
      <param name="gateway-url" value="http://127.0.0.1:8000/api/v1/freeswitch/directory" bindings="directory"/>
      <param name="method" value="POST"/>
      <param name="timeout" value="10"/>
    </binding>
  </bindings>
</configuration>
EOF
```

### 6.6 Iniciar FreeSWITCH

```bash
systemctl start freeswitch

# Verificar
fs_cli -x "status"
fs_cli -x "sofia status"
```

---

## 7. RTPEngine

### 7.1 Instalar RTPEngine

```bash
# Si dSIPRouter no lo instaló, hacerlo manualmente:
apt install -y \
    debhelper \
    iptables-dev \
    libcurl4-openssl-dev \
    libglib2.0-dev \
    libhiredis-dev \
    libpcre3-dev \
    libssl-dev \
    markdown \
    zlib1g-dev \
    libavcodec-dev \
    libavfilter-dev \
    libavformat-dev \
    libavutil-dev \
    libswresample-dev \
    libevent-dev \
    libjson-glib-dev \
    libpcap-dev

# Clonar y compilar
cd /usr/src
git clone https://github.com/sipwise/rtpengine.git
cd rtpengine
dpkg-buildpackage -us -uc -b
cd ..
dpkg -i rtpengine*.deb
```

### 7.2 Configurar RTPEngine

```bash
cat > /etc/rtpengine/rtpengine.conf << 'EOF'
[rtpengine]
table = 0
interface = 10.10.22.4
listen-ng = 127.0.0.1:7722
port-min = 10000
port-max = 20000
log-level = 6
log-facility = daemon
log-facility-cdr = local0
log-facility-rtcp = local1
EOF
```

### 7.3 Iniciar RTPEngine

```bash
systemctl enable rtpengine
systemctl start rtpengine

# Verificar
rtpengine-ctl list numsessions
```

---

## 8. Rust Backend

### 8.1 Instalar Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env

rustc --version  # >= 1.75.0
```

### 8.2 Clonar código fuente

```bash
cd /opt
git clone https://github.com/TU_USUARIO/ApoloBilling.git
cd ApoloBilling
```

### 8.3 Configurar variables de entorno

```bash
cat > /opt/ApoloBilling/rust-backend/.env << 'EOF'
# Database
DATABASE_URL=postgresql://apolo_user:TU_PASSWORD_SEGURO_PG@localhost:5432/apolo_billing
DATABASE_MAX_CONNECTIONS=20

# Server
RUST_SERVER_HOST=0.0.0.0
RUST_SERVER_PORT=8000
RUST_SERVER_WORKERS=4

# CORS
CORS_ORIGINS=http://localhost:3000,http://TU_IP_PUBLICA:3000

# JWT (generar con: openssl rand -hex 32)
JWT_SECRET=TU_JWT_SECRET_ALEATORIO_DE_64_CARACTERES
JWT_EXPIRATION_SECS=1800

# Redis
REDIS_URL=redis://127.0.0.1:6379

# Kamailio MySQL (opcional)
KAMAILIO_DATABASE_URL=mysql://kamailio:TU_PASSWORD_MYSQL@localhost:3306/kamailio

# Logging
RUST_LOG=apolo_billing=info,apolo_api=info,actix_web=info
EOF
```

### 8.4 Ejecutar migraciones de base de datos

```bash
cd /opt/ApoloBilling

# Schema principal
sudo -u postgres psql -d apolo_billing -f database/schema.sql

# Migraciones (en orden)
# 002 - Planes de cuenta
# 003 - Dispositivos SIP y tablas adicionales
# 004 - Sistema de routing unificado (trunks, inbound/outbound routes)
# 005 - Tipos de trunk (public/private) y estado SIP OPTIONS
# 006 - Rutas internas (FreeSWITCH ↔ Kamailio)
# 007 - Transformación de números en rutas inbound
# 008 - Destino de trunk en rutas inbound
# 009 - Early media en rutas inbound

for f in database/migrations/*.sql; do
    echo "Ejecutando: $f"
    sudo -u postgres psql -d apolo_billing -f "$f"
done

# Datos base (opcional - solo para instalación nueva)
sudo -u postgres psql -d apolo_billing -f database/seeds/base_data.sql
```

### 8.5 Crear usuario administrador

```bash
# Primero compilamos el backend para poder generar el hash
cd /opt/ApoloBilling/rust-backend
cargo build --release

# El hash para "admin123" en Argon2id es:
# (En producción, usar un password más seguro)
sudo -u postgres psql -d apolo_billing << 'EOF'
INSERT INTO usuarios (username, password_hash, full_name, role, active)
VALUES (
    'admin',
    '$argon2id$v=19$m=19456,t=2,p=1$YWJjZGVmZ2hpamtsbW5vcA$8K1TqNwGVdPCIJL0hFz8qLqkU8b5HjZm7RIJqK5hI/E',
    'Administrador',
    'superadmin',
    true
) ON CONFLICT (username) DO NOTHING;
EOF
```

### 8.6 Compilar backend

```bash
cd /opt/ApoloBilling/rust-backend
cargo build --release

# Verificar binario
ls -lh target/release/apolo-billing
```

---

## 9. Rust Billing Engine

### 9.1 Configurar variables de entorno

```bash
cat > /opt/ApoloBilling/rust-billing-engine/.env << 'EOF'
# Environment
ENVIRONMENT=production

# Server
HOST=0.0.0.0
PORT=9000

# Database
DATABASE_URL=postgresql://apolo_user:TU_PASSWORD_SEGURO_PG@localhost:5432/apolo_billing

# Redis
REDIS_URL=redis://127.0.0.1:6379

# FreeSWITCH ESL
FREESWITCH_SERVERS=127.0.0.1:8021:ClueCon

# Logging
RUST_LOG=info,apolo_billing_engine=debug
EOF
```

### 9.2 Compilar billing engine

```bash
cd /opt/ApoloBilling/rust-billing-engine
cargo build --release

ls -lh target/release/apolo-billing-engine
```

---

## 10. Frontend React

### 10.1 Instalar Node.js 20

```bash
curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
apt install -y nodejs

node --version  # >= 20.x
npm --version
```

### 10.2 Compilar frontend

```bash
cd /opt/ApoloBilling/frontend

# Instalar dependencias
npm install

# Build para producción
npm run build

# Verificar
ls -la dist/
```

---

## 11. Nginx

### 11.1 Instalar Nginx

```bash
apt install -y nginx

systemctl enable nginx
```

### 11.2 Configurar sitio ApoloBilling

```bash
cat > /etc/nginx/sites-available/apolobilling << 'EOF'
# Redirect HTTP to HTTPS (descomentar cuando tengas SSL)
# server {
#     listen 80;
#     server_name tu-dominio.com;
#     return 301 https://$server_name$request_uri;
# }

server {
    listen 80;
    listen [::]:80;
    # listen 443 ssl http2;  # Descomentar para HTTPS

    server_name _;

    # SSL (descomentar cuando tengas certificados)
    # ssl_certificate /etc/letsencrypt/live/tu-dominio.com/fullchain.pem;
    # ssl_certificate_key /etc/letsencrypt/live/tu-dominio.com/privkey.pem;

    root /opt/ApoloBilling/frontend/dist;
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
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
    }

    # WebSocket
    location /ws {
        proxy_pass http://127.0.0.1:8000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_read_timeout 86400s;
    }

    # SPA fallback
    location / {
        try_files $uri $uri/ /index.html;
    }

    # Logs
    access_log /var/log/nginx/apolobilling.access.log;
    error_log /var/log/nginx/apolobilling.error.log;
}
EOF

# Habilitar sitio
ln -sf /etc/nginx/sites-available/apolobilling /etc/nginx/sites-enabled/
rm -f /etc/nginx/sites-enabled/default

# Verificar y reiniciar
nginx -t
systemctl restart nginx
```

### 11.3 SSL con Let's Encrypt (opcional pero recomendado)

```bash
apt install -y certbot python3-certbot-nginx

certbot --nginx -d tu-dominio.com

# Renovación automática
systemctl enable certbot.timer
```

---

## 12. Servicios Systemd

### 12.1 Backend Service

```bash
cat > /etc/systemd/system/apolo-backend.service << 'EOF'
[Unit]
Description=Apolo Backend - REST API
After=network.target postgresql.service redis.service
Wants=postgresql.service redis.service

[Service]
Type=simple
User=root
WorkingDirectory=/opt/ApoloBilling/rust-backend
EnvironmentFile=/opt/ApoloBilling/rust-backend/.env
ExecStart=/opt/ApoloBilling/rust-backend/target/release/apolo-billing
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF
```

### 12.2 Billing Engine Service

```bash
cat > /etc/systemd/system/apolo-billing-engine.service << 'EOF'
[Unit]
Description=Apolo Billing Engine - Real-time billing
After=network.target postgresql.service redis.service freeswitch.service
Wants=postgresql.service redis.service

[Service]
Type=simple
User=root
WorkingDirectory=/opt/ApoloBilling/rust-billing-engine
EnvironmentFile=/opt/ApoloBilling/rust-billing-engine/.env
ExecStart=/opt/ApoloBilling/rust-billing-engine/target/release/apolo-billing-engine
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF
```

### 12.3 Habilitar e iniciar servicios

```bash
systemctl daemon-reload

# Iniciar servicios
systemctl enable apolo-backend apolo-billing-engine
systemctl start apolo-backend apolo-billing-engine

# Verificar estado
systemctl status apolo-backend
systemctl status apolo-billing-engine
```

---

## 13. Integración Final

### 13.1 Configurar FreeSWITCH como endpoint en Kamailio

```bash
mysql -u kamailio -p kamailio << 'EOF'
-- Agregar FreeSWITCH como gateway PBX (type=9)
INSERT INTO dr_gateways (gwid, type, address, strip, pri_prefix, attrs, description)
VALUES (100, 9, '127.0.0.1:5080', 0, '', '100,9', 'FreeSWITCH PBX');

-- Crear grupo
INSERT INTO dr_gw_lists (id, gwlist, description)
VALUES (100, '100', 'name:FreeSWITCH_PBX,type:9');

-- Mapeo
INSERT INTO dsip_gw2gwgroup (gwid, gwgroupid)
VALUES (100, 100);

-- ACL
INSERT INTO address (grp, ip_addr, mask, port, tag)
VALUES (9, '127.0.0.1', 32, 5080, 'name:FreeSWITCH_PBX,gwgroup:100');
EOF

# Recargar Kamailio
kamcmd drouting.reload
kamcmd permissions.addressReload
```

### 13.2 Configurar gateway a Kamailio en FreeSWITCH

```bash
cat > /etc/freeswitch/sip_profiles/external/kamailio.xml << 'EOF'
<include>
  <gateway name="kamailio">
    <param name="realm" value="127.0.0.1"/>
    <param name="proxy" value="127.0.0.1:5060"/>
    <param name="register" value="false"/>
    <param name="caller-id-in-from" value="true"/>
  </gateway>
</include>
EOF

# Recargar FreeSWITCH
fs_cli -x "sofia profile external rescan"
```

---

## 14. Sistema de Routing Unificado

El sistema de routing unificado permite gestionar rutas inbound y outbound desde una única interfaz, con soporte para múltiples trunks y failover automático.

### 14.1 Conceptos Clave

**Tipos de Trunk:**
- **Public (Kamailio)**: Carriers externos gestionados via dr_gateways (type=8)
- **Private (FreeSWITCH)**: Trunks internos configurados en FreeSWITCH

**Grupos de Trunks:**
- Permiten agrupar múltiples trunks para failover
- Priorización configurable (menor número = mayor prioridad)
- Peso para distribución de carga

### 14.2 Configurar Rutas Outbound

Las rutas outbound determinan cómo se enrutan las llamadas salientes basándose en prefijos.

```bash
# Acceder via API o desde el frontend en /routing/outbound

# Ejemplo de ruta outbound via API:
curl -X POST http://localhost:8000/api/v1/routing/outbound-routes \
  -H "Content-Type: application/json" \
  -H "Cookie: auth_token=..." \
  -d '{
    "name": "Argentina Mobile",
    "prefix": "5411",
    "trunk_group_id": 1,
    "priority": 10,
    "strip_digits": 0,
    "prepend_digits": "",
    "enabled": true
  }'
```

### 14.3 Configurar Rutas Inbound

Las rutas inbound determinan cómo se procesan las llamadas entrantes según el DID.

```bash
# Características de rutas inbound:
# - Pattern matching con wildcards (X = 0-9, N = 2-9, . = cualquier cantidad)
# - Transformación de número (destination_transform)
# - Destino a trunk específico o grupo
# - Early media configurable

# Ejemplo via API:
curl -X POST http://localhost:8000/api/v1/routing/inbound-routes \
  -H "Content-Type: application/json" \
  -H "Cookie: auth_token=..." \
  -d '{
    "name": "DID Principal",
    "did_pattern": "5411XXXXXXXX",
    "destination": "100",
    "destination_type": "extension",
    "destination_transform": "",
    "early_media": false,
    "enabled": true
  }'
```

### 14.4 Sincronizar con Kamailio

Los trunks públicos se sincronizan automáticamente con las tablas de Kamailio:

```bash
# Las tablas afectadas son:
# - dr_gateways (gateways/carriers)
# - dr_gw_lists (grupos de gateways)
# - dr_rules (reglas de routing)
# - address (ACL por IP)

# Recargar Kamailio después de cambios manuales:
kamcmd drouting.reload
kamcmd permissions.addressReload
kamcmd htable.reload gw2gwgroup
```

### 14.5 Endpoints API de Routing

| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/api/v1/routing/trunks` | GET/POST | Listar/crear trunks |
| `/api/v1/routing/trunks/{id}` | GET/PUT/DELETE | CRUD trunk individual |
| `/api/v1/routing/trunk-groups` | GET/POST | Grupos de trunks |
| `/api/v1/routing/trunk-groups/{id}/members` | GET/POST | Miembros del grupo |
| `/api/v1/routing/outbound-routes` | GET/POST | Rutas outbound |
| `/api/v1/routing/inbound-routes` | GET/POST | Rutas inbound |
| `/api/v1/routing/sip-status` | GET | Estado SIP de peers |
| `/api/v1/routing/sync-kamailio` | POST | Sincronizar con Kamailio |

---

## 15. Dispositivos SIP y Monitoreo

### 15.1 Gestión de Dispositivos SIP

Los dispositivos SIP (extensiones, softphones, ATAs) se gestionan desde el frontend en `/sip-devices`.

```bash
# Crear dispositivo SIP via API:
curl -X POST http://localhost:8000/api/v1/sip-devices \
  -H "Content-Type: application/json" \
  -H "Cookie: auth_token=..." \
  -d '{
    "username": "1001",
    "password": "SecurePass123!",
    "domain": "pbx.example.com",
    "display_name": "Recepción",
    "account_id": 1,
    "enabled": true,
    "codecs": "PCMA,PCMU,G729",
    "max_calls": 2
  }'
```

### 15.2 Autenticación Dinámica con mod_xml_curl

FreeSWITCH consulta al backend para autenticar dispositivos:

```bash
# El endpoint /api/v1/freeswitch/directory responde con XML:
# - Soporta REGISTER y INVITE
# - Usa A1 hash (MD5) para autenticación
# - Retorna configuración de codecs y caller ID

# Verificar funcionamiento:
curl -X POST http://localhost:8000/api/v1/freeswitch/directory \
  -d "action=sip_auth&user=1001&domain=pbx.example.com&sip_auth_method=REGISTER"
```

### 15.3 Monitoreo SIP OPTIONS

El sistema puede monitorear el estado de los peers SIP usando SIP OPTIONS:

```bash
# Configurar monitoreo en system_settings:
sudo -u postgres psql -d apolo_billing << 'EOF'
INSERT INTO system_settings (key, value, description)
VALUES
  ('sip_options_enabled', 'true', 'Habilitar monitoreo SIP OPTIONS'),
  ('sip_options_interval', '30', 'Intervalo en segundos entre verificaciones')
ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value;
EOF
```

**Estados de SIP:**
- `online`: Peer respondió OK (200)
- `offline`: Sin respuesta o timeout
- `unknown`: No verificado aún

### 15.4 Verificar Estado de Peers

```bash
# Ver estado actual de todos los peers:
curl -s http://localhost:8000/api/v1/routing/sip-status | jq

# El estado se guarda en routing_sip_status_log con:
# - trunk_id
# - sip_status (online/offline/unknown)
# - last_check
# - response_time_ms
# - error_message (si aplica)
```

### 15.5 Lista de IPs Permitidas para FreeSWITCH

```bash
# Agregar IP a la whitelist del endpoint directory:
sudo -u postgres psql -d apolo_billing << 'EOF'
INSERT INTO freeswitch_allowed_ips (ip_address, description, enabled)
VALUES ('10.0.0.100', 'FreeSWITCH Primary', true);
EOF
```

---

## 16. Rutas Internas

Las rutas internas definen la interconexión entre FreeSWITCH y Kamailio dentro del sistema.

### 16.1 System Endpoints

Los endpoints del sistema representan los puntos de conexión internos:

```bash
# Crear endpoint de sistema:
sudo -u postgres psql -d apolo_billing << 'EOF'
INSERT INTO system_endpoints (name, endpoint_type, host, port, transport, enabled, description)
VALUES
  ('FreeSWITCH Internal', 'freeswitch', '127.0.0.1', 5080, 'udp', true, 'FreeSWITCH internal profile'),
  ('FreeSWITCH External', 'freeswitch', '127.0.0.1', 5062, 'udp', true, 'FreeSWITCH external profile'),
  ('Kamailio SBC', 'kamailio', '127.0.0.1', 5060, 'udp', true, 'Kamailio SIP proxy');
EOF
```

### 16.2 Rutas Internas

```bash
# Ejemplo de ruta: FreeSWITCH → Kamailio
sudo -u postgres psql -d apolo_billing << 'EOF'
INSERT INTO internal_routes (
  name,
  route_type,
  source_endpoint_id,
  destination_endpoint_id,
  context,
  match_pattern,
  priority,
  enabled
)
VALUES (
  'Outbound to Kamailio',
  'fs_to_kamailio',
  1,  -- FreeSWITCH Internal
  3,  -- Kamailio SBC
  'from-pbx',
  '^[0-9]+$',
  10,
  true
);
EOF
```

### 16.3 Tipos de Rutas

| Tipo | Descripción |
|------|-------------|
| `fs_to_kamailio` | Llamadas de FreeSWITCH hacia carriers via Kamailio |
| `kamailio_to_fs` | Llamadas entrantes de Kamailio hacia FreeSWITCH |
| `fs_internal` | Llamadas internas dentro de FreeSWITCH |

### 16.4 API de Rutas Internas

| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/api/v1/internal-routes` | GET/POST | Listar/crear rutas |
| `/api/v1/internal-routes/{id}` | GET/PUT/DELETE | CRUD individual |
| `/api/v1/internal-routes/endpoints` | GET/POST | Endpoints del sistema |

---

## 17. Verificación

### 17.1 Script de verificación completa

```bash
#!/bin/bash
echo "=========================================="
echo "  VERIFICACIÓN DE APOLOBILLING STACK"
echo "=========================================="

echo -e "\n[1] PostgreSQL..."
pg_isready && echo "✓ PostgreSQL OK" || echo "✗ PostgreSQL FAIL"

echo -e "\n[2] Redis..."
redis-cli ping | grep -q PONG && echo "✓ Redis OK" || echo "✗ Redis FAIL"

echo -e "\n[3] MySQL/MariaDB..."
mysqladmin ping -u root -p 2>/dev/null && echo "✓ MySQL OK" || echo "✗ MySQL FAIL"

echo -e "\n[4] Kamailio..."
kamcmd dispatcher.list >/dev/null 2>&1 && echo "✓ Kamailio OK" || echo "✗ Kamailio FAIL"

echo -e "\n[5] FreeSWITCH..."
fs_cli -x "status" >/dev/null 2>&1 && echo "✓ FreeSWITCH OK" || echo "✗ FreeSWITCH FAIL"

echo -e "\n[6] RTPEngine..."
rtpengine-ctl list numsessions >/dev/null 2>&1 && echo "✓ RTPEngine OK" || echo "✗ RTPEngine FAIL"

echo -e "\n[7] Backend API..."
curl -s http://localhost:8000/api/v1/health | grep -q ok && echo "✓ Backend OK" || echo "✗ Backend FAIL"

echo -e "\n[8] Billing Engine..."
curl -s http://localhost:9000/api/v1/health | grep -q ok && echo "✓ Billing Engine OK" || echo "✗ Billing Engine FAIL"

echo -e "\n[9] Nginx..."
curl -sI http://localhost | grep -q "200\|301\|302" && echo "✓ Nginx OK" || echo "✗ Nginx FAIL"

echo -e "\n[10] Routing System..."
curl -s http://localhost:8000/api/v1/routing/trunks -H "Cookie: auth_token=$TOKEN" | grep -q "data" && echo "✓ Routing OK" || echo "✗ Routing FAIL (need auth)"

echo -e "\n[11] SIP Devices..."
curl -s http://localhost:8000/api/v1/sip-devices -H "Cookie: auth_token=$TOKEN" | grep -q "data" && echo "✓ SIP Devices OK" || echo "✗ SIP Devices FAIL (need auth)"

echo -e "\n[12] FreeSWITCH Directory Endpoint..."
curl -s -X POST http://localhost:8000/api/v1/freeswitch/directory -d "action=sip_auth" | grep -q "xml" && echo "✓ FS Directory OK" || echo "✗ FS Directory FAIL"

echo -e "\n=========================================="
echo "  PUERTOS EN USO"
echo "=========================================="
ss -tlnp | grep -E '(5060|5080|5062|8000|9000|80|443|3306|5432|6379|8021)'

echo -e "\n=========================================="
echo "  TABLAS DE ROUTING"
echo "=========================================="
sudo -u postgres psql -d apolo_billing -c "SELECT 'routing_trunks' as tabla, count(*) FROM routing_trunks UNION ALL SELECT 'routing_inbound_routes', count(*) FROM routing_inbound_routes UNION ALL SELECT 'routing_outbound_routes', count(*) FROM routing_outbound_routes UNION ALL SELECT 'sip_devices', count(*) FROM sip_devices;"
```

### 17.2 Logs útiles

```bash
# Ver todos los logs en tiempo real
journalctl -f -u apolo-backend -u apolo-billing-engine -u freeswitch -u kamailio

# Solo backend
journalctl -u apolo-backend -f

# Solo billing engine
journalctl -u apolo-billing-engine -f

# FreeSWITCH
tail -f /var/log/freeswitch/freeswitch.log

# Kamailio
tail -f /var/log/kamailio/kamailio.log
```

### 17.3 Acceder al sistema

1. Abrir navegador: `http://TU_IP_SERVIDOR`
2. Login: `admin` / `admin123`
3. Cambiar password inmediatamente

---

## Diagrama Final de Arquitectura

```
                                   INTERNET
                                       │
                                       ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│                              SERVIDOR DEBIAN 12                              │
│                                                                              │
│  ┌────────────────────┐                                                      │
│  │   Nginx :80/:443   │───► Frontend React (dist/)                          │
│  │   (Reverse Proxy)  │                                                      │
│  └─────────┬──────────┘                                                      │
│            │                                                                 │
│            │ /api/* → :8000                                                  │
│            ▼                                                                 │
│  ┌────────────────────┐         ┌────────────────────┐                      │
│  │  Rust Backend      │────────►│  PostgreSQL :5432  │◄────┐                │
│  │     :8000          │         │  (apolo_billing)   │     │                │
│  └────────────────────┘         └────────────────────┘     │                │
│                                                             │                │
│  ┌────────────────────┐         ┌────────────────────┐     │                │
│  │  Billing Engine    │────────►│    Redis :6379     │     │                │
│  │     :9000          │         └────────────────────┘     │                │
│  └─────────┬──────────┘                                    │                │
│            │ ESL :8021                                     │                │
│            ▼                                               │                │
│  ┌────────────────────┐         ┌────────────────────┐     │                │
│  │   FreeSWITCH       │────────►│  MySQL :3306       │     │                │
│  │  :5080 (internal)  │         │  (kamailio)        │     │                │
│  │  :5062 (external)  │         └─────────┬──────────┘     │                │
│  └─────────┬──────────┘                   │                │                │
│            │                              │                │                │
│            │ SIP                          │                │                │
│            ▼                              ▼                │                │
│  ┌────────────────────┐         ┌────────────────────┐     │                │
│  │   Kamailio SBC     │────────►│   RTPEngine        │     │                │
│  │      :5060         │         │  :10000-20000      │     │                │
│  └─────────┬──────────┘         └────────────────────┘     │                │
│            │                                               │                │
└────────────┼───────────────────────────────────────────────┼────────────────┘
             │                                               │
             ▼                                               │
     ┌───────────────┐                              Billing via API
     │   CARRIERS    │                                       │
     │  (VOIPSWITCH) │◄──────────────────────────────────────┘
     └───────────────┘
```

---

## Resumen de Credenciales (cambiar en producción)

| Servicio | Usuario | Password | Notas |
|----------|---------|----------|-------|
| PostgreSQL | apolo_user | TU_PASSWORD_SEGURO_PG | Base: apolo_billing |
| MySQL | kamailio | TU_PASSWORD_MYSQL | Base: kamailio |
| Redis | - | - | Sin auth por defecto |
| FreeSWITCH ESL | - | ClueCon | Puerto 8021 |
| ApoloBilling Web | admin | admin123 | Cambiar inmediatamente |
| dSIPRouter | admin | (definido en instalación) | Puerto 5000 |

---

## Troubleshooting

### El backend no inicia

```bash
# Verificar logs
journalctl -u apolo-backend -n 50 --no-pager

# Causas comunes:
# 1. DATABASE_URL mal configurada
# 2. PostgreSQL no está corriendo
# 3. Puerto 8000 ya en uso
ss -tlnp | grep 8000
```

### El billing engine no se conecta a FreeSWITCH

```bash
# Verificar que ESL está disponible
nc -zv 127.0.0.1 8021

# Verificar password ESL
fs_cli -x "status"

# Verificar logs del billing engine
journalctl -u apolo-billing-engine -n 50 --no-pager | grep -i "connect\|error\|fail"
```

### Frontend muestra página en blanco

```bash
# Verificar que dist/ existe y tiene contenido
ls -la /opt/ApoloBilling/frontend/dist/

# Verificar configuración Nginx
nginx -t
cat /etc/nginx/sites-enabled/apolobilling

# Verificar que el proxy al backend funciona
curl -s http://localhost:8000/api/v1/health
```

### Kamailio no enruta llamadas

```bash
# Verificar gateways
kamcmd drouting.list

# Verificar dispatcher
kamcmd dispatcher.list

# Verificar permisos
kamcmd permissions.addressDump

# Recargar configuración
kamcmd cfg.reload
```

### FreeSWITCH no registra dispositivos

```bash
# Verificar profiles
fs_cli -x "sofia status"

# Verificar que mod_xml_curl responde
curl -X POST http://localhost:8000/api/v1/freeswitch/directory \
    -d "action=sip_auth&user=test&domain=test"

# Verificar logs
tail -f /var/log/freeswitch/freeswitch.log | grep -i "auth\|register"
```

### Rutas outbound no funcionan

```bash
# Verificar que existen rutas configuradas
sudo -u postgres psql -d apolo_billing -c "SELECT name, prefix, enabled FROM routing_outbound_routes;"

# Verificar grupos de trunk
sudo -u postgres psql -d apolo_billing -c "SELECT tg.name, t.name as trunk, t.enabled FROM routing_trunk_groups tg JOIN routing_trunk_group_members m ON tg.id = m.trunk_group_id JOIN routing_trunks t ON m.trunk_id = t.id;"

# Verificar sincronización con Kamailio
kamcmd drouting.list

# Forzar sincronización
curl -X POST http://localhost:8000/api/v1/routing/sync-kamailio -H "Cookie: auth_token=..."
```

### SIP OPTIONS no funciona

```bash
# Verificar configuración
sudo -u postgres psql -d apolo_billing -c "SELECT * FROM system_settings WHERE key LIKE 'sip_options%';"

# Verificar estado de peers
sudo -u postgres psql -d apolo_billing -c "SELECT t.name, s.sip_status, s.last_check FROM routing_trunks t LEFT JOIN routing_sip_status_log s ON t.id = s.trunk_id;"

# Verificar logs del backend
journalctl -u apolo-backend -f | grep -i "sip\|options"
```

### Dispositivos SIP no aparecen en FreeSWITCH

```bash
# Verificar que el dispositivo existe y está habilitado
sudo -u postgres psql -d apolo_billing -c "SELECT username, domain, enabled FROM sip_devices;"

# Verificar IP whitelist
sudo -u postgres psql -d apolo_billing -c "SELECT * FROM freeswitch_allowed_ips WHERE enabled = true;"

# Probar endpoint manualmente
curl -X POST http://localhost:8000/api/v1/freeswitch/directory \
    -d "action=sip_auth&user=1001&domain=pbx.example.com&sip_auth_method=REGISTER"

# Verificar respuesta XML válida
# Debe contener <document type="freeswitch/xml">
```

---

---

## Endpoints API Completos

### Autenticación y Usuarios
| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/api/v1/auth/login` | POST | Login (retorna JWT en cookie) |
| `/api/v1/auth/logout` | POST | Logout |
| `/api/v1/auth/me` | GET | Usuario actual |
| `/api/v1/auth/change-password` | POST | Cambiar password |
| `/api/v1/users` | GET/POST | Listar/crear usuarios |

### Cuentas y Billing
| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/api/v1/accounts` | GET/POST | Listar/crear cuentas |
| `/api/v1/accounts/{id}` | GET/PUT | Ver/actualizar cuenta |
| `/api/v1/accounts/{id}/topup` | POST | Recargar saldo |
| `/api/v1/plans` | GET/POST | Planes de cuenta |
| `/api/v1/reservations` | GET | Reservas activas |

### CDRs y Llamadas
| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/api/v1/cdrs` | GET | Lista de CDRs (paginado) |
| `/api/v1/cdrs/export` | GET | Exportar CSV/JSON |
| `/api/v1/cdrs/stats` | GET | Estadísticas |
| `/api/v1/active-calls` | GET | Llamadas en curso |

### Tarifas
| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/api/v1/rate-cards` | GET/POST | Tarjetas de tarifa |
| `/api/v1/rate-cards/search/{phone}` | GET | Búsqueda LPM |
| `/api/v1/rates/zonas` | GET/POST | Zonas de tarifa |
| `/api/v1/rates/prefijos` | GET/POST | Prefijos |
| `/api/v1/rates/tarifas` | GET/POST | Tarifas |

### Routing Unificado
| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/api/v1/routing/trunks` | GET/POST | Trunks (públicos/privados) |
| `/api/v1/routing/trunks/{id}` | GET/PUT/DELETE | CRUD trunk |
| `/api/v1/routing/trunk-groups` | GET/POST | Grupos de trunk |
| `/api/v1/routing/trunk-groups/{id}` | GET/PUT/DELETE | CRUD grupo |
| `/api/v1/routing/trunk-groups/{id}/members` | GET/POST/DELETE | Miembros |
| `/api/v1/routing/outbound-routes` | GET/POST | Rutas outbound |
| `/api/v1/routing/outbound-routes/{id}` | GET/PUT/DELETE | CRUD outbound |
| `/api/v1/routing/inbound-routes` | GET/POST | Rutas inbound |
| `/api/v1/routing/inbound-routes/{id}` | GET/PUT/DELETE | CRUD inbound |
| `/api/v1/routing/sip-status` | GET | Estado SIP OPTIONS |
| `/api/v1/routing/sync-kamailio` | POST | Sincronizar Kamailio |

### Dispositivos SIP
| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/api/v1/sip-devices` | GET/POST | Listar/crear dispositivos |
| `/api/v1/sip-devices/{id}` | GET/PUT/DELETE | CRUD dispositivo |
| `/api/v1/sip-devices/{id}/status` | GET | Estado de registro |

### FreeSWITCH
| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/api/v1/freeswitch/directory` | POST | Autenticación dinámica (mod_xml_curl) |
| `/api/v1/freeswitch/allowed-ips` | GET/POST | IPs permitidas |

### Rutas Internas
| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/api/v1/internal-routes` | GET/POST | Rutas internas |
| `/api/v1/internal-routes/{id}` | GET/PUT/DELETE | CRUD ruta |
| `/api/v1/internal-routes/endpoints` | GET/POST | Endpoints del sistema |

### Kamailio Dialplan
| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/api/v1/kamailio-dialplan/groups` | GET/POST | Grupos de carrier |
| `/api/v1/kamailio-dialplan/carriers` | GET/POST | Carriers (gateways) |
| `/api/v1/kamailio-dialplan/routes` | GET/POST | Reglas de routing |
| `/api/v1/kamailio-endpoints` | GET/POST | Endpoints PBX (type=9) |

### Sistema
| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/api/v1/health` | GET | Health check |
| `/api/v1/stats` | GET | Dashboard stats |
| `/api/v1/settings` | GET/PUT | Configuración sistema |
| `/api/v1/audit-logs` | GET | Logs de auditoría |
| `/ws` | WebSocket | Updates en tiempo real |

---

## Comandos Útiles

```bash
# Estado de todos los servicios
systemctl status apolo-backend apolo-billing-engine freeswitch kamailio rtpengine nginx postgresql redis mariadb

# Reiniciar todo el stack
systemctl restart apolo-backend apolo-billing-engine

# Ver conexiones activas
ss -tlnp | grep -E '(5060|5080|8000|9000)'

# Verificar llamadas activas en FreeSWITCH
fs_cli -x "show calls"

# Verificar sesiones RTP
rtpengine-ctl list numsessions

# Monitorear tráfico SIP
ngrep -d any -W byline port 5060

# Ver trunks configurados
sudo -u postgres psql -d apolo_billing -c "SELECT name, trunk_type, host, port, enabled FROM routing_trunks;"

# Ver rutas outbound
sudo -u postgres psql -d apolo_billing -c "SELECT name, prefix, priority, enabled FROM routing_outbound_routes ORDER BY priority;"

# Ver dispositivos SIP
sudo -u postgres psql -d apolo_billing -c "SELECT username, domain, display_name, enabled FROM sip_devices;"

# Ver estado SIP OPTIONS
sudo -u postgres psql -d apolo_billing -c "SELECT t.name, s.sip_status, s.last_check, s.response_time_ms FROM routing_trunks t LEFT JOIN routing_sip_status_log s ON t.id = s.trunk_id ORDER BY s.last_check DESC;"

# Forzar recarga de Kamailio
kamcmd drouting.reload && kamcmd permissions.addressReload

# Ver registros activos en FreeSWITCH
fs_cli -x "sofia status profile internal reg"

# Debug de autenticación FreeSWITCH
fs_cli -x "sofia loglevel all 9"
tail -f /var/log/freeswitch/freeswitch.log | grep -i auth
```

---

## Páginas del Frontend

| Ruta | Descripción | Rol Mínimo |
|------|-------------|------------|
| `/` | Dashboard con estadísticas | operator |
| `/accounts` | Gestión de cuentas/clientes | operator |
| `/cdr` | Registros de llamadas (CDR) | operator |
| `/active-calls` | Llamadas en curso | operator |
| `/balance` | Movimientos de saldo | operator |
| `/rates` | Gestión de tarifas | operator |
| `/zones` | Zonas de tarificación | operator |
| `/plans` | Planes de cuenta | admin |
| `/sip-devices` | **Dispositivos SIP** | admin |
| `/routing` | **Routing Unificado** (Inbound/Outbound) | admin |
| `/internal-routing` | **Rutas Internas** | admin |
| `/kamailio-dialplan` | **Dialplan Kamailio** | superadmin |
| `/dialplan` | Dialplan FreeSWITCH | superadmin |
| `/users` | Gestión de usuarios | superadmin |
| `/audit-logs` | Logs de auditoría | superadmin |

---

## Changelog de Migraciones

| Migración | Descripción |
|-----------|-------------|
| 002 | Tabla de planes de cuenta |
| 003 | Dispositivos SIP y tablas base adicionales |
| 004 | Sistema routing unificado (trunks, grupos, rutas inbound/outbound) |
| 005 | Tipos de trunk (public/private) y log de SIP OPTIONS |
| 006 | Rutas internas y endpoints del sistema |
| 007 | Campo destination_transform en rutas inbound |
| 008 | Campo trunk_destination_id en rutas inbound |
| 009 | Campo early_media en rutas inbound |
