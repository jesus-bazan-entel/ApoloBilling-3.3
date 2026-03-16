# Guia de Despliegue - ApoloBilling v2

Guia paso a paso para desplegar ApoloBilling en un servidor Debian/Ubuntu con FreeSWITCH.

## Indice

1. [Requisitos del Servidor](#1-requisitos-del-servidor)
2. [Instalacion de Dependencias](#2-instalacion-de-dependencias)
3. [Base de Datos PostgreSQL](#3-base-de-datos-postgresql)
4. [Redis](#4-redis)
5. [Codigo Fuente](#5-codigo-fuente)
6. [Compilar Rust Backend](#6-compilar-rust-backend)
7. [Compilar Billing Engine](#7-compilar-billing-engine)
8. [Compilar Frontend](#8-compilar-frontend)
9. [Configurar Nginx](#9-configurar-nginx)
10. [Configurar Servicios Systemd](#10-configurar-servicios-systemd)
11. [Primer Inicio y Verificacion](#11-primer-inicio-y-verificacion)
12. [Conectar con FreeSWITCH](#12-conectar-con-freeswitch)
13. [Monitoreo y Logs](#13-monitoreo-y-logs)
14. [Actualizaciones](#14-actualizaciones)
15. [Troubleshooting](#15-troubleshooting)

---

## 1. Requisitos del Servidor

### Hardware Minimo
| Recurso | Minimo | Recomendado |
|---------|--------|-------------|
| CPU | 2 cores | 4+ cores |
| RAM | 2 GB | 4+ GB |
| Disco | 20 GB SSD | 50+ GB SSD |

### Software Requerido
| Software | Version | Proposito |
|----------|---------|-----------|
| Debian/Ubuntu | 12+ / 22.04+ | Sistema operativo |
| PostgreSQL | 15+ | Base de datos principal |
| Redis | 7+ | Cache y sesiones |
| Rust | 1.75+ | Compilar backend y billing engine |
| Node.js | 20+ | Compilar frontend |
| Nginx | 1.24+ | Reverse proxy y servidor web |

### Puertos
| Puerto | Servicio | Exposicion |
|--------|----------|------------|
| 3000 | Nginx (Frontend + Proxy) | Publico (o via firewall) |
| 8000 | Rust Backend API | Solo localhost |
| 9000 | Billing Engine API | Solo localhost |
| 5432 | PostgreSQL | Solo localhost |
| 6379 | Redis | Solo localhost |
| 8021 | FreeSWITCH ESL | Solo localhost |

---

## 2. Instalacion de Dependencias

### 2.1 Actualizar sistema

```bash
sudo apt update && sudo apt upgrade -y
```

### 2.2 Instalar herramientas base

```bash
sudo apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    curl \
    git \
    wget \
    unzip
```

### 2.3 Instalar Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env

# Verificar
rustc --version    # >= 1.75.0
cargo --version
```

### 2.4 Instalar Node.js 20

```bash
# Usando NodeSource
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install -y nodejs

# Verificar
node --version     # >= 20.x
npm --version

# Instalar serve (para servir el frontend en produccion)
sudo npm install -g serve
```

### 2.5 Instalar PostgreSQL 15

```bash
sudo apt install -y postgresql-15 postgresql-contrib-15

# Verificar que esta corriendo
sudo systemctl status postgresql
sudo systemctl enable postgresql
```

### 2.6 Instalar Redis 7

```bash
sudo apt install -y redis-server

# Verificar
sudo systemctl status redis-server
sudo systemctl enable redis-server
redis-cli ping    # Debe responder PONG
```

### 2.7 Instalar Nginx

```bash
sudo apt install -y nginx

sudo systemctl enable nginx
```

---

## 3. Base de Datos PostgreSQL

### 3.1 Crear usuario y base de datos

```bash
sudo -u postgres psql
```

```sql
-- Crear usuario
CREATE USER apolo_user WITH PASSWORD 'TU_PASSWORD_SEGURO_AQUI';

-- Crear base de datos
CREATE DATABASE apolo_billing OWNER apolo_user;

-- Permisos
GRANT ALL PRIVILEGES ON DATABASE apolo_billing TO apolo_user;

-- Conectar a la base
\c apolo_billing

-- Dar permisos en el schema public
GRANT ALL ON SCHEMA public TO apolo_user;

\q
```

### 3.2 Ejecutar schema inicial

```bash
cd /opt/ApoloBillingv2

# Schema base (tablas, tipos enum, indices, triggers)
sudo -u postgres psql -d apolo_billing -f database/schema.sql

# Migraciones
sudo -u postgres psql -d apolo_billing -f database/migration.sql
sudo -u postgres psql -d apolo_billing -f database/migrations/002_add_plans_table.sql
```

### 3.3 Crear tablas adicionales del sistema

```bash
sudo -u postgres psql -d apolo_billing
```

```sql
-- Tabla de usuarios del sistema (si no existe en schema.sql)
CREATE TABLE IF NOT EXISTS usuarios (
    id SERIAL PRIMARY KEY,
    username VARCHAR(100) UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    full_name VARCHAR(255),
    role VARCHAR(20) NOT NULL DEFAULT 'operator'
        CHECK (role IN ('superadmin', 'admin', 'operator')),
    active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Tabla de llamadas activas
CREATE TABLE IF NOT EXISTS active_calls (
    call_id VARCHAR(255) PRIMARY KEY,
    calling_number VARCHAR(50),
    called_number VARCHAR(50),
    direction VARCHAR(20),
    start_time TIMESTAMPTZ,
    answer_time TIMESTAMPTZ,
    status VARCHAR(20) DEFAULT 'ringing',
    current_duration INTEGER DEFAULT 0,
    current_cost DECIMAL(12,4) DEFAULT 0,
    server VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Tabla de CDRs (si el schema original usa otro nombre)
CREATE TABLE IF NOT EXISTS cdrs (
    id BIGSERIAL PRIMARY KEY,
    call_uuid VARCHAR(255) NOT NULL,
    account_id INTEGER REFERENCES accounts(id),
    caller_number VARCHAR(50),
    called_number VARCHAR(50),
    start_time TIMESTAMPTZ,
    answer_time TIMESTAMPTZ,
    end_time TIMESTAMPTZ,
    duration INTEGER DEFAULT 0,
    billsec INTEGER DEFAULT 0,
    hangup_cause VARCHAR(100),
    rate_per_minute DECIMAL(12,6),
    cost DECIMAL(12,6),
    direction VARCHAR(20),
    freeswitch_server_id VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Tabla de logs de auditoria
CREATE TABLE IF NOT EXISTS audit_logs (
    id BIGSERIAL PRIMARY KEY,
    user_id INTEGER,
    action VARCHAR(100) NOT NULL,
    entity_type VARCHAR(50),
    entity_id VARCHAR(100),
    details JSONB,
    ip_address VARCHAR(45),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Tabla de settings del sistema
CREATE TABLE IF NOT EXISTS system_settings (
    key VARCHAR(100) PRIMARY KEY,
    value TEXT NOT NULL,
    description TEXT,
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Setting por defecto: autorizacion outbound habilitada
INSERT INTO system_settings (key, value, description)
VALUES ('require_outbound_authorization', 'true', 'Si es true, las llamadas outbound requieren autorizacion y reserva de saldo')
ON CONFLICT (key) DO NOTHING;

-- Tabla de zonas
CREATE TABLE IF NOT EXISTS zonas (
    id SERIAL PRIMARY KEY,
    nombre VARCHAR(255) NOT NULL,
    descripcion TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Tabla de prefijos
CREATE TABLE IF NOT EXISTS prefijos (
    id SERIAL PRIMARY KEY,
    zona_id INTEGER REFERENCES zonas(id),
    prefijo VARCHAR(20) NOT NULL,
    descripcion VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Tabla de tarifas
CREATE TABLE IF NOT EXISTS tarifas (
    id SERIAL PRIMARY KEY,
    zona_id INTEGER REFERENCES zonas(id),
    nombre VARCHAR(255),
    precio_por_minuto DECIMAL(12,6) NOT NULL,
    incremento INTEGER DEFAULT 6,
    vigencia_inicio TIMESTAMPTZ DEFAULT NOW(),
    vigencia_fin TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Tabla de dialplan
CREATE TABLE IF NOT EXISTS dialplan_entries (
    id SERIAL PRIMARY KEY,
    context VARCHAR(100) NOT NULL,
    extension_name VARCHAR(100) NOT NULL,
    priority INTEGER DEFAULT 100,
    condition_field VARCHAR(100),
    condition_expression TEXT,
    actions JSONB NOT NULL,
    enabled BOOLEAN DEFAULT TRUE,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Indices para performance
CREATE INDEX IF NOT EXISTS idx_cdrs_account_id ON cdrs(account_id);
CREATE INDEX IF NOT EXISTS idx_cdrs_start_time ON cdrs(start_time);
CREATE INDEX IF NOT EXISTS idx_cdrs_call_uuid ON cdrs(call_uuid);
CREATE INDEX IF NOT EXISTS idx_cdrs_direction ON cdrs(direction);
CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs(created_at);
CREATE INDEX IF NOT EXISTS idx_active_calls_direction ON active_calls(direction);

\q
```

### 3.4 Crear usuario administrador inicial

```bash
# Necesitaras el hash Argon2 del password.
# Opcion 1: Usar el ejemplo incluido en el backend
cd /opt/ApoloBillingv2/rust-backend
# Si existe el ejemplo gen_hash, compilar y usar
# Opcion 2: Insertar directamente (password: admin123)
sudo -u postgres psql -d apolo_billing -c "
INSERT INTO usuarios (username, password_hash, full_name, role)
VALUES (
    'admin',
    '\$argon2id\$v=19\$m=19456,t=2,p=1\$PASSWORD_HASH_AQUI',
    'Administrador',
    'superadmin'
) ON CONFLICT (username) DO NOTHING;
"
```

> **Nota:** Para generar el hash Argon2, puedes compilar y ejecutar el backend con la ruta de registro POST `/api/v1/auth/register` despues de crear un primer superadmin manualmente, o usar una herramienta como `argon2` CLI.

### 3.5 Verificar conexion

```bash
psql -U apolo_user -h localhost -d apolo_billing -c "SELECT COUNT(*) FROM rate_cards;"
```

---

## 4. Redis

Redis viene configurado por defecto. Solo verificar:

```bash
redis-cli ping           # PONG
redis-cli info memory    # Verificar memoria disponible
```

Para produccion, editar `/etc/redis/redis.conf`:

```bash
# Limitar memoria
maxmemory 256mb
maxmemory-policy allkeys-lru

# Deshabilitar persistencia si no se necesita
# save ""
```

```bash
sudo systemctl restart redis-server
```

---

## 5. Codigo Fuente

### 5.1 Clonar repositorio

```bash
cd /opt
git clone https://github.com/jesus-bazan-entel/ApoloBillingv2.git
cd ApoloBillingv2
```

### 5.2 Verificar estructura

```bash
ls -la
# Debe mostrar: frontend/ rust-backend/ rust-billing-engine/ database/ deploy/
```

---

## 6. Compilar Rust Backend

### 6.1 Configurar environment

```bash
cd /opt/ApoloBillingv2/rust-backend
cp .env.example .env
```

Editar `.env`:

```bash
nano .env
```

```env
DATABASE_URL=postgresql://apolo_user:TU_PASSWORD@localhost:5432/apolo_billing
DATABASE_MAX_CONNECTIONS=20
RUST_SERVER_HOST=0.0.0.0
RUST_SERVER_PORT=8000
RUST_SERVER_WORKERS=4
CORS_ORIGINS=http://localhost:3000,http://TU_IP:3000
JWT_SECRET=GENERA_UN_SECRET_ALEATORIO_LARGO_AQUI
RUST_LOG=apolo_billing=info,apolo_api=info,actix_web=info
```

> **Tip:** Generar JWT_SECRET: `openssl rand -hex 32`

### 6.2 Compilar

```bash
cargo build --release
```

> La primera compilacion tarda 3-8 minutos. Las siguientes son incrementales y mas rapidas.

### 6.3 Verificar compilacion

```bash
ls -lh target/release/apolo-billing
# Debe existir el binario (~15-30 MB)
```

---

## 7. Compilar Billing Engine

### 7.1 Configurar environment

```bash
cd /opt/ApoloBillingv2/rust-billing-engine
cp .env.example .env
nano .env
```

```env
ENVIRONMENT=production
HOST=0.0.0.0
PORT=9000
DATABASE_URL=postgresql://apolo_user:TU_PASSWORD@localhost:5432/apolo_billing
REDIS_URL=redis://127.0.0.1:6379

# PRODUCCION: Conectar al FreeSWITCH real
FREESWITCH_SERVERS=127.0.0.1:8021:ClueCon

# TESTING: Dejar vacio para modo servidor ESL
# FREESWITCH_SERVERS=

RUST_LOG=info,apolo_billing_engine=debug
```

### 7.2 Compilar

```bash
cargo build --release
```

### 7.3 Verificar

```bash
ls -lh target/release/apolo-billing-engine
```

---

## 8. Compilar Frontend

### 8.1 Instalar dependencias

```bash
cd /opt/ApoloBillingv2/frontend
npm install
```

### 8.2 Build para produccion

```bash
npm run build
```

El build genera la carpeta `dist/` con los archivos estaticos:

```bash
ls dist/
# index.html  assets/
```

---

## 9. Configurar Nginx

### 9.1 Copiar configuracion

```bash
sudo cp /opt/ApoloBillingv2/deploy/nginx/apolo-frontend.conf /etc/nginx/sites-available/
sudo ln -sf /etc/nginx/sites-available/apolo-frontend.conf /etc/nginx/sites-enabled/

# Remover default si existe
sudo rm -f /etc/nginx/sites-enabled/default
```

### 9.2 Ajustar configuracion (si es necesario)

```bash
sudo nano /etc/nginx/sites-available/apolo-frontend.conf
```

Puntos clave de la configuracion:

```nginx
server {
    listen 3000;                              # Puerto publico
    root /opt/ApoloBillingv2/frontend/dist;   # Archivos estaticos del frontend

    # Proxy API al backend Rust
    location /api/ {
        proxy_pass http://127.0.0.1:8000;     # Backend en puerto 8000
    }

    # WebSocket
    location /ws {
        proxy_pass http://127.0.0.1:8000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }

    # SPA fallback
    location / {
        try_files $uri $uri/ /index.html;
    }
}
```

> **Para HTTPS:** Agregar certificados SSL con Let's Encrypt (`certbot --nginx`) y cambiar listen a 443 ssl.

### 9.3 Verificar y reiniciar

```bash
sudo nginx -t                    # Test configuracion
sudo systemctl restart nginx
sudo systemctl enable nginx
```

---

## 10. Configurar Servicios Systemd

### 10.1 Copiar archivos de servicio

```bash
sudo cp /opt/ApoloBillingv2/deploy/systemd/apolo-backend.service /etc/systemd/system/
sudo cp /opt/ApoloBillingv2/deploy/systemd/apolo-billing-engine.service /etc/systemd/system/
```

> **Nota:** Si usas la ruta `/opt/ApoloBillingv2/` en vez de `/opt/ApoloBilling/`, editar las rutas en los archivos .service:

```bash
sudo sed -i 's|/opt/ApoloBilling/|/opt/ApoloBillingv2/|g' /etc/systemd/system/apolo-backend.service
sudo sed -i 's|/opt/ApoloBilling/|/opt/ApoloBillingv2/|g' /etc/systemd/system/apolo-billing-engine.service
```

### 10.2 Verificar contenido de los servicios

**apolo-backend.service:**
```ini
[Unit]
Description=Apolo Backend - REST API
After=network.target postgresql.service redis.service

[Service]
Type=simple
User=root
WorkingDirectory=/opt/ApoloBillingv2/rust-backend
Environment=RUST_LOG=info,apolo_api=debug,apolo_billing=debug
ExecStart=/opt/ApoloBillingv2/rust-backend/target/release/apolo-billing
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

**apolo-billing-engine.service:**
```ini
[Unit]
Description=Apolo Billing Engine - Real-time billing
After=network.target postgresql.service redis.service

[Service]
Type=simple
User=root
WorkingDirectory=/opt/ApoloBillingv2/rust-billing-engine
Environment=RUST_LOG=info,apolo_billing_engine=debug
ExecStart=/opt/ApoloBillingv2/rust-billing-engine/target/release/apolo-billing-engine
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### 10.3 Habilitar e iniciar servicios

```bash
sudo systemctl daemon-reload

# Iniciar servicios
sudo systemctl start apolo-backend
sudo systemctl start apolo-billing-engine

# Habilitar inicio automatico
sudo systemctl enable apolo-backend
sudo systemctl enable apolo-billing-engine

# Verificar estado
sudo systemctl status apolo-backend
sudo systemctl status apolo-billing-engine
```

---

## 11. Primer Inicio y Verificacion

### 11.1 Verificar health del backend

```bash
curl -s http://localhost:8000/api/v1/health | jq .
# Debe responder: {"status":"ok"} o similar
```

### 11.2 Verificar frontend

Abrir en el navegador: `http://TU_IP:3000`

Debe mostrar la pagina de login.

### 11.3 Verificar billing engine

```bash
curl -s http://localhost:9000/api/v1/health | jq .
```

### 11.4 Verificar logs

```bash
# Backend
journalctl -u apolo-backend -f --no-pager -n 20

# Billing Engine
journalctl -u apolo-billing-engine -f --no-pager -n 20
```

### 11.5 Crear primer usuario (si no se hizo antes)

Si el backend tiene la ruta de registro abierta temporalmente, puedes crear el primer superadmin via API. De lo contrario, insertar directamente en la base de datos.

---

## 12. Conectar con FreeSWITCH

### 12.1 Requisitos FreeSWITCH

El billing engine se conecta via ESL (Event Socket Layer). Verificar que FreeSWITCH tenga ESL habilitado:

```bash
# En el servidor FreeSWITCH
cat /etc/freeswitch/autoload_configs/event_socket.conf.xml
```

Debe contener:

```xml
<configuration name="event_socket.conf" description="Socket Client">
  <settings>
    <param name="nat-map" value="false"/>
    <param name="listen-ip" value="0.0.0.0"/>
    <param name="listen-port" value="8021"/>
    <param name="password" value="ClueCon"/>
    <param name="apply-inbound-acl" value="loopback.auto"/>
  </settings>
</configuration>
```

### 12.2 Configurar billing engine

Editar `/opt/ApoloBillingv2/rust-billing-engine/.env`:

```env
# Un solo servidor FreeSWITCH
FREESWITCH_SERVERS=192.168.1.10:8021:ClueCon

# Multiples servidores (separados por coma)
FREESWITCH_SERVERS=192.168.1.10:8021:ClueCon,192.168.1.11:8021:ClueCon
```

### 12.3 Reiniciar billing engine

```bash
sudo systemctl restart apolo-billing-engine
journalctl -u apolo-billing-engine -f
# Debe mostrar: "Connected and authenticated to FreeSWITCH"
```

### 12.4 Eventos que procesa el billing engine

| Evento ESL | Accion |
|---|---|
| `CHANNEL_CREATE` | Autorizar llamada, crear reserva |
| `CHANNEL_ANSWER` | Iniciar billing en tiempo real |
| `CHANNEL_HANGUP_COMPLETE` | Generar CDR, consumir reserva |

---

## 13. Monitoreo y Logs

### 13.1 Comandos utiles

```bash
# Estado de todos los servicios
systemctl status apolo-backend apolo-billing-engine nginx postgresql redis-server

# Logs en tiempo real
journalctl -u apolo-backend -f
journalctl -u apolo-billing-engine -f

# Logs de Nginx
tail -f /var/log/nginx/apolo-frontend.access.log
tail -f /var/log/nginx/apolo-frontend.error.log

# Conexiones activas al backend
ss -tlnp | grep -E '(3000|8000|9000)'

# Verificar PostgreSQL
sudo -u postgres psql -d apolo_billing -c "SELECT COUNT(*) as total_cdrs FROM cdrs;"
sudo -u postgres psql -d apolo_billing -c "SELECT COUNT(*) as active_calls FROM active_calls;"

# Verificar Redis
redis-cli info keyspace
redis-cli keys "call_*"
redis-cli keys "setting:*"
```

### 13.2 Health checks

```bash
# Script rapido de health check
echo "=== Backend ===" && curl -s http://localhost:8000/api/v1/health && echo
echo "=== Billing Engine ===" && curl -s http://localhost:9000/api/v1/health && echo
echo "=== PostgreSQL ===" && pg_isready && echo
echo "=== Redis ===" && redis-cli ping
echo "=== Nginx ===" && curl -sI http://localhost:3000 | head -1
```

---

## 14. Actualizaciones

### 14.1 Procedimiento de actualizacion

```bash
cd /opt/ApoloBillingv2

# 1. Obtener cambios
git pull origin main

# 2. Compilar backend
cd rust-backend && cargo build --release && cd ..

# 3. Compilar billing engine
cd rust-billing-engine && cargo build --release && cd ..

# 4. Compilar frontend
cd frontend && npm install && npm run build && cd ..

# 5. Reiniciar servicios
sudo systemctl restart apolo-backend
sudo systemctl restart apolo-billing-engine
# Nginx no necesita reinicio (sirve archivos estaticos de dist/)

# 6. Verificar
curl -s http://localhost:8000/api/v1/health
journalctl -u apolo-backend -n 5 --no-pager
journalctl -u apolo-billing-engine -n 5 --no-pager
```

### 14.2 Rollback

```bash
# Si algo falla, volver al commit anterior
cd /opt/ApoloBillingv2
git log --oneline -5          # Ver commits recientes
git checkout COMMIT_ANTERIOR

# Recompilar y reiniciar
cd rust-backend && cargo build --release && cd ..
cd rust-billing-engine && cargo build --release && cd ..
cd frontend && npm run build && cd ..
sudo systemctl restart apolo-backend apolo-billing-engine
```

---

## 15. Troubleshooting

### El backend no inicia

```bash
# Verificar logs
journalctl -u apolo-backend -n 50 --no-pager

# Causas comunes:
# 1. DATABASE_URL mal configurada
# 2. PostgreSQL no esta corriendo
# 3. Puerto 8000 ya en uso
ss -tlnp | grep 8000
```

### El billing engine no se conecta a FreeSWITCH

```bash
# Verificar que ESL esta disponible
nc -zv 127.0.0.1 8021

# Verificar password ESL
fs_cli -x "status"

# Verificar logs del billing engine
journalctl -u apolo-billing-engine -n 50 --no-pager | grep -i "connect\|error\|fail"
```

### Frontend muestra pagina en blanco

```bash
# Verificar que dist/ existe y tiene contenido
ls -la /opt/ApoloBillingv2/frontend/dist/

# Verificar configuracion Nginx
sudo nginx -t
cat /etc/nginx/sites-enabled/apolo-frontend.conf

# Verificar que el proxy al backend funciona
curl -s http://localhost:8000/api/v1/health
```

### CDRs sin costo (Cost = $0.00)

```bash
# Verificar si hay rate_cards configuradas
sudo -u postgres psql -d apolo_billing -c "SELECT COUNT(*) FROM rate_cards WHERE effective_start <= NOW() AND (effective_end IS NULL OR effective_end >= NOW());"

# Verificar setting de autorizacion
sudo -u postgres psql -d apolo_billing -c "SELECT * FROM system_settings WHERE key = 'require_outbound_authorization';"

# Si la autorizacion esta desactivada, el CDR generator hace LPM lookup
# automaticamente para calcular el costo. Verificar en logs:
journalctl -u apolo-billing-engine | grep "LPM billing"
```

### Redis no responde

```bash
sudo systemctl restart redis-server
redis-cli ping

# Si esta lleno:
redis-cli info memory
redis-cli flushdb    # CUIDADO: borra todas las keys
```

### Conexion a PostgreSQL rechazada

```bash
# Verificar pg_hba.conf
sudo cat /etc/postgresql/15/main/pg_hba.conf | grep apolo

# Debe tener una linea como:
# host  apolo_billing  apolo_user  127.0.0.1/32  scram-sha-256

# Si no existe, agregar y reiniciar
sudo systemctl restart postgresql
```

---

## Diagrama de Despliegue

```
+------------------------------------------------------------------+
|                    SERVIDOR PRODUCCION                             |
|                                                                    |
|  +-------------------+    +--------------------+                   |
|  | systemd           |    | systemd            |                   |
|  | apolo-backend     |    | apolo-billing-     |                   |
|  |   :8000            |    | engine :9000       |                   |
|  +--------+----------+    +--------+-----------+                   |
|           |                        |                               |
|           +----------+-------------+                               |
|                      |                                             |
|               +------+-------+                                     |
|               | PostgreSQL   |     +-------------+                 |
|               | :5432        |     | Redis :6379 |                 |
|               +--------------+     +-------------+                 |
|                                                                    |
|  +-------------------+                                             |
|  | Nginx :3000       |----> frontend/dist/ (archivos estaticos)    |
|  |  /api/* -> :8000  |                                             |
|  |  /ws    -> :8000  |                                             |
|  +-------------------+                                             |
|                                                                    |
|  +-------------------+                                             |
|  | FreeSWITCH        |<---- ESL :8021 <---- Billing Engine         |
|  | (PBX)             |                                             |
|  +-------------------+                                             |
+------------------------------------------------------------------+
```

---

## Resumen de Archivos de Configuracion

| Archivo | Ubicacion | Proposito |
|---------|-----------|-----------|
| `.env` | `rust-backend/` | Config backend (DB, JWT, CORS) |
| `.env` | `rust-billing-engine/` | Config billing (DB, Redis, FreeSWITCH) |
| `apolo-backend.service` | `/etc/systemd/system/` | Servicio backend |
| `apolo-billing-engine.service` | `/etc/systemd/system/` | Servicio billing |
| `apolo-frontend.conf` | `/etc/nginx/sites-available/` | Nginx reverse proxy |

Todos los templates estan en `deploy/` dentro del repositorio.
