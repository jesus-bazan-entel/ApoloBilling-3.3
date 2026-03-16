# ApoloBilling v2

Plataforma de tarificacion en tiempo real para centrales telefonicas FreeSWITCH. Gestiona autorizacion de llamadas, reservas de saldo, billing en tiempo real y generacion de CDRs (Call Detail Records).

## Arquitectura del Sistema

```
                        +--------------------------+
                        |     USUARIOS / BROWSER   |
                        +-----------+--------------+
                                    |
                                    | HTTPS :3000
                                    v
                    +-------------------------------+
                    |     Nginx (Reverse Proxy)     |
                    |  - Sirve frontend (SPA)       |
                    |  - Proxy /api/ -> :8000       |
                    |  - Proxy /ws   -> :8000       |
                    +-------+-------------+---------+
                            |             |
                     /api/* |             | /ws
                            v             v
                  +----------------------------+
                  |    Rust Backend (:8000)     |
                  |    [Actix-web 4]            |
                  |                            |
                  |  - Auth (JWT + Argon2)     |
                  |  - Accounts CRUD + Topup   |
                  |  - Rate Cards (LPM)        |
                  |  - CDRs + Export CSV/JSON  |
                  |  - Active Calls            |
                  |  - Plans / Users / Audit   |
                  |  - Dialplan Management     |
                  |  - WebSocket (tiempo real) |
                  +-------+----------+---------+
                          |          |
              +-----------+          +----------+
              v                                 v
  +-------------------+             +-------------------+
  | PostgreSQL 15     |             | Redis 7           |
  |                   |             |                   |
  | - accounts        |             | - Cache de rates  |
  | - rate_cards      |             | - Sesiones JWT    |
  | - cdrs            |             | - Call sessions   |
  | - reservations    |             | - Settings cache  |
  | - plans           |             +-------------------+
  | - usuarios        |
  | - active_calls    |
  | - audit_logs      |
  +-------------------+
              ^
              |
  +----------------------------+          +-------------------+
  | Rust Billing Engine (:9000)|  <---->  | FreeSWITCH PBX    |
  |                            |   ESL    |                   |
  | - Autorizacion de llamadas |          | - CHANNEL_CREATE  |
  | - Reserva de saldo         |          | - CHANNEL_ANSWER  |
  | - Billing en tiempo real   |          | - CHANNEL_HANGUP  |
  | - Generacion de CDRs       |          +-------------------+
  | - Simulador de llamadas    |
  +----------------------------+
```

### Tres Componentes Principales

| Componente | Puerto | Tecnologia | Funcion |
|---|---|---|---|
| **Frontend** | 3000 | React 19 + TypeScript + Vite 7 | SPA con TanStack Query, Radix UI, Tailwind CSS |
| **Rust Backend** | 8000 | Actix-web 4 (Cargo workspace, 7 crates) | API REST, Auth, CRUD, CDR queries, WebSocket |
| **Billing Engine** | 9000 | Tokio + ESL (FreeSWITCH Event Socket) | Autorizacion, billing real-time, CDR generation |

## Estructura del Proyecto

```
ApoloBillingv2/
|
+-- frontend/                          # React SPA
|   +-- src/
|   |   +-- api/client.ts             # Cliente Axios (todos los endpoints)
|   |   +-- components/               # Layout, DataTable, StatCard, Badge
|   |   +-- pages/                    # Dashboard, CDR, Accounts, Rates, Login...
|   |   +-- hooks/useWebSocket.ts     # WebSocket para actualizaciones en vivo
|   |   +-- types/index.ts            # Interfaces TypeScript
|   |   +-- lib/                      # Utilidades
|   |   +-- App.tsx                   # Router + QueryClient + rutas protegidas
|   |   +-- main.tsx                  # Entry point
|   +-- vite.config.ts                # Proxy /api -> :8000 en desarrollo
|   +-- package.json
|
+-- rust-backend/                      # API Server (Cargo Workspace)
|   +-- src/main.rs                   # Setup Actix-web, rutas, middleware
|   +-- crates/
|   |   +-- apolo-api/               # Handlers HTTP + DTOs
|   |   |   +-- src/handlers/        # auth, account, cdr, rate_card, dialplan...
|   |   |   +-- src/dto/             # Request/Response types
|   |   +-- apolo-auth/              # JWT (HTTP-only cookies) + Argon2
|   |   +-- apolo-cache/             # Redis caching layer
|   |   +-- apolo-core/              # Modelos compartidos, config, errores
|   |   +-- apolo-db/                # Repositorios PostgreSQL (SQLx 0.8)
|   |   +-- apolo-esl/               # FreeSWITCH ESL integration
|   |   +-- apolo-services/          # Logica de negocio (rating, billing sync)
|   +-- .env.example
|
+-- rust-billing-engine/               # Motor de Billing en Tiempo Real
|   +-- src/
|   |   +-- services/
|   |   |   +-- authorization.rs      # Autorizacion + LPM rate lookup
|   |   |   +-- realtime_biller.rs    # Monitoreo cada 180s, extension de reservas
|   |   |   +-- cdr_generator.rs      # Creacion de CDR, calculo de costo
|   |   |   +-- reservation_manager.rs # Reservas de saldo CRUD
|   |   |   +-- call_simulator.rs     # Simulador para testing
|   |   +-- esl/
|   |   |   +-- event_handler.rs      # Procesa eventos FreeSWITCH
|   |   |   +-- client.rs             # Modo produccion (conecta a FS)
|   |   |   +-- server.rs             # Modo testing (acepta conexiones)
|   |   +-- models/                   # Account, RateCard, CDR, Reservation
|   |   +-- api/                      # HTTP API del billing engine
|   |   +-- cache/                    # Cliente Redis
|   |   +-- database/                 # Pool PostgreSQL (deadpool)
|   +-- Dockerfile
|   +-- .env.example
|
+-- database/                          # Schema y Migraciones
|   +-- schema.sql                    # Schema completo inicial
|   +-- migration.sql                 # Migraciones de columnas
|   +-- migrations/
|       +-- 002_add_plans_table.sql   # Tabla de planes
|
+-- deploy/                            # Configuraciones de Despliegue
|   +-- systemd/                      # Servicios systemd
|   |   +-- apolo-backend.service
|   |   +-- apolo-billing-engine.service
|   |   +-- apolo-frontend.service
|   +-- nginx/
|       +-- apolo-frontend.conf       # Nginx reverse proxy config
|
+-- .github/workflows/                # CI/CD
|   +-- ci.yml                        # Build + Lint + Test
|   +-- deploy.yml                    # Deploy a produccion
|   +-- security.yml                  # Auditorias de seguridad
|
+-- DEPLOY.md                         # Guia de despliegue detallada
+-- README.md                         # Este archivo
```

## Flujo de Tarificacion

```
 Llamada Entrante al PBX
         |
         v
 1. CHANNEL_CREATE (FreeSWITCH -> Billing Engine via ESL)
         |
         +---> Determinar direccion:
         |       - from-pbx = OUTBOUND (se cobra al caller)
         |       - to-kamailio = INBOUND (no se cobra, excepto 0800)
         |       - 0800/0801/1800 = TOLL-FREE (se cobra al dueño del numero)
         |
         +---> Autorizacion (si esta habilitada):
         |       - Buscar cuenta por ANI (caller number)
         |       - LPM: Longest Prefix Match en rate_cards
         |       - Verificar saldo disponible
         |       - Crear reserva de saldo
         |       - Si falla -> uuid_kill (cortar llamada)
         |
         v
 2. CHANNEL_ANSWER
         |
         +---> Iniciar RealtimeBiller
         |       - Monitoreo periodico cada 180 segundos
         |       - Extender reserva si es necesario
         |
         v
 3. CHANNEL_HANGUP_COMPLETE
         |
         +---> CdrGenerator:
         |       - Calcular costo real (billsec x rate/min)
         |       - Aplicar billing_increment (redondeo, ej: 6s)
         |       - Consumir reserva (debitar saldo real)
         |       - Liberar excedente de reserva
         |       - Insertar CDR en base de datos
         |
         v
     CDR Generado
     (visible en la web en tiempo real via WebSocket)
```

### Calculo de Reserva Inicial

```
base     = rate_per_minute x 5 minutos
buffer   = base x 8%
total    = clamp(base + buffer, $0.30, $30.00)
max_secs = (total / rate_per_minute) x 60
```

### LPM (Longest Prefix Match)

Para el destino `541156000`, se generan prefijos:
```
["541156000", "54115600", "5411560", "541156", "54115", "5411", "541", "54", "5"]
```
Se busca en `rate_cards` el prefijo mas largo que coincida, ordenado por prioridad.

## API REST (Puerto 8000)

Todas las rutas bajo `/api/v1/`. Autenticacion via JWT en cookies HTTP-only.

| Endpoint | Metodo | Descripcion | Acceso |
|---|---|---|---|
| `/health` | GET | Health check | Publico |
| `/auth/login` | POST | Login (setea cookie JWT) | Publico |
| `/auth/me` | GET | Info del usuario actual | Autenticado |
| `/accounts` | GET/POST | Listar/crear cuentas | Autenticado |
| `/accounts/{id}` | GET/PUT | Obtener/actualizar cuenta | Autenticado |
| `/accounts/{id}/topup` | POST | Recargar saldo | Autenticado |
| `/rate-cards` | GET/POST | Listar/crear tarifas | Autenticado |
| `/rate-cards/search/{phone}` | GET | Buscar tarifa por LPM | Autenticado |
| `/cdrs` | GET | Listar CDRs con paginacion | Autenticado |
| `/cdrs/export` | GET | Exportar CDRs (CSV/JSON streaming) | Autenticado |
| `/cdrs/stats` | GET | Estadisticas de CDRs | Autenticado |
| `/active-calls` | GET | Llamadas activas | Autenticado |
| `/plans/*` | CRUD | Gestion de planes | Autenticado |
| `/users/*` | CRUD | Gestion de usuarios | Superadmin |
| `/dialplan/*` | CRUD | Dialplan FreeSWITCH | Superadmin |
| `/audit-logs/*` | GET | Logs de auditoria | Superadmin |
| `/ws` | WebSocket | Updates en tiempo real | Autenticado |

**Roles:** `superadmin` > `admin` > `operator`

## Desarrollo Local

### Prerequisitos

- **Rust** >= 1.75 (con cargo)
- **Node.js** >= 20 (con npm)
- **PostgreSQL** >= 15
- **Redis** >= 7

### 1. Base de datos

```bash
sudo -u postgres createdb apolo_billing
sudo -u postgres createuser apolo_user -P  # Definir password
sudo -u postgres psql -d apolo_billing -f database/schema.sql
sudo -u postgres psql -d apolo_billing -f database/migration.sql
sudo -u postgres psql -d apolo_billing -f database/migrations/002_add_plans_table.sql
```

### 2. Backend API

```bash
cd rust-backend
cp .env.example .env   # Editar con credenciales reales
cargo build --release
cargo run --release     # Inicia en :8000
```

### 3. Billing Engine

```bash
cd rust-billing-engine
cp .env.example .env   # Editar con credenciales reales
cargo build --release
cargo run --release     # Inicia en :9000

# FREESWITCH_SERVERS vacio = modo testing (ESL server en :8021)
# FREESWITCH_SERVERS=host:port:password = modo produccion (ESL client)
```

### 4. Frontend

```bash
cd frontend
npm install
npm run dev            # Dev server en :3000 (proxy a :8000)
```

Abrir http://localhost:3000

### Comandos Utiles

```bash
# Rust Backend
cargo test                             # Tests
cargo fmt --all -- --check             # Format check
cargo clippy -- -D warnings            # Lint

# Billing Engine
cargo test --test full_integration_test  # Integration tests

# Frontend
npm run build                          # Build produccion -> dist/
npm run lint                           # ESLint
```

## Variables de Entorno

| Variable | Default | Descripcion |
|---|---|---|
| `DATABASE_URL` | - | PostgreSQL connection string |
| `REDIS_URL` | `redis://localhost:6379` | Redis connection |
| `RUST_SERVER_PORT` | `8000` | Puerto del backend API |
| `CORS_ORIGINS` | `http://localhost:3000` | Origenes permitidos |
| `JWT_SECRET` | - | Secreto para tokens JWT |
| `JWT_EXPIRATION_SECS` | `1800` | Expiracion JWT (30 min) |
| `ENVIRONMENT` | `development` | Entorno del billing engine |
| `PORT` | `9000` | Puerto del billing engine |
| `FREESWITCH_SERVERS` | (vacio) | Modo testing vs produccion |
| `RUST_LOG` | `info` | Nivel de logging |

## Patrones Importantes

### Calculos Financieros
Siempre usar `rust_decimal::Decimal` (nunca `f64`) para valores monetarios.

### Timestamps
Siempre usar `DateTime<Utc>` (no `NaiveDateTime`) para columnas `TIMESTAMP WITH TIME ZONE`.

### Paginacion API
```json
{
  "data": [...],
  "pagination": {
    "total": 100,
    "page": 1,
    "per_page": 20,
    "total_pages": 5
  }
}
```

### WebSocket
El frontend recibe updates en tiempo real via `/ws`:
- `active_calls` - Lista completa de llamadas activas
- `call_start` / `call_update` / `call_end` - Eventos individuales
- `stats_update` - Estadisticas del dashboard

## Licencia

Propiedad de Entel. Uso interno.
