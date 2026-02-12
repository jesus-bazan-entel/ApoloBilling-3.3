# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

ApoloBilling is a real-time telecommunications billing platform for FreeSWITCH PBX environments. It handles call authorization, balance reservations, real-time billing, and CDR (Call Detail Records) generation.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      React Frontend (:3000)                      │
│                    (Proxy → localhost:8000)                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Rust Backend (:8000)                        │
│  - Auth (JWT + Argon2)        - CDRs (queries, export, stats)   │
│  - Accounts CRUD + Topup      - Active Calls                    │
│  - Rate Cards CRUD + LPM      - Reservations                    │
│  - Zones/Prefixes/Tariffs     - Dashboard Stats                 │
│  - Plans / Users / Audit      - Dialplan Management             │
└─────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
┌─────────────────────────────┐     ┌─────────────────────────────┐
│   Rust Billing Engine       │     │   PostgreSQL + Redis        │
│        (:9000)              │     │                             │
│   ESL, real-time billing    │     │                             │
└─────────────────────────────┘     └─────────────────────────────┘
```

**Three main components:**
- **frontend/** - React 19 + TypeScript + Vite 7 SPA with TanStack Query, Radix UI, Tailwind CSS 4
- **rust-backend/** - Actix-web 4 API server (Cargo workspace with 7 crates) for all CRUD, auth, CDR queries
- **rust-billing-engine/** - Real-time billing processor via FreeSWITCH ESL (dual-mode: ESL server for testing, ESL client for production)

## Common Commands

### Rust Backend
```bash
cd rust-backend
cargo build --release             # Build for production
cargo run --release               # Start server (port from RUST_SERVER_PORT env, default 9001)
cargo test                        # Run all tests
cargo test <test_name>            # Run single test
cargo fmt --all -- --check        # Format check (CI runs this)
cargo clippy -- -D warnings       # Lint check (CI runs this)
```

### Frontend
```bash
cd frontend
npm install                       # Install dependencies
npm run dev                       # Dev server on :3000 (proxies /api and /ws to :8000)
npm run build                     # Production build → dist/
npm run lint                      # ESLint check
```

### Rust Billing Engine
```bash
cd rust-billing-engine
cargo build --release
cargo run                         # Start billing engine on :9000
cargo test                        # Run all tests
cargo test --test full_integration_test   # Integration tests
```

### Systemd Services (Production)
```bash
systemctl status apolo-backend           # Backend API
systemctl status apolo-billing-engine    # Billing Engine
systemctl status apolo-frontend          # Frontend
journalctl -u apolo-backend -f           # Follow backend logs
```

## Code Structure

### Rust Backend (Cargo Workspace)
```
rust-backend/
├── src/main.rs               # Server setup, all route configuration, middleware
└── crates/
    ├── apolo-api/            # HTTP handlers and DTOs
    │   ├── handlers/         # Endpoint handlers (auth, account, cdr, rate_card, dialplan, etc.)
    │   └── dto/              # Request/response types
    ├── apolo-db/             # PostgreSQL repositories (SQLx 0.8)
    ├── apolo-auth/           # JWT (HTTP-only cookies) + Argon2 authentication
    ├── apolo-cache/          # Redis caching layer
    ├── apolo-core/           # Shared models, traits, error handling, configuration
    ├── apolo-services/       # Business logic services
    └── apolo-esl/            # FreeSWITCH Event Socket Layer integration
```

### Rust Billing Engine
```
rust-billing-engine/src/
├── services/                 # Core business logic
│   ├── authorization.rs      # Call authorization, LPM rate lookup
│   ├── realtime_biller.rs    # Active call monitoring, reservation extensions (every 180s)
│   ├── cdr_generator.rs      # CDR creation, cost calculation
│   └── reservation_manager.rs # Balance reservation CRUD
├── esl/                      # FreeSWITCH Event Socket Layer
│   └── event_handler.rs      # ESL event processing
├── api/                      # HTTP API routes
├── database/                 # PostgreSQL queries (tokio-postgres + deadpool)
└── cache.rs                  # Redis client
```

### Frontend
```
frontend/src/
├── pages/                    # Route components (Dashboard, CDR, Accounts, Rates, Dialplan, etc.)
├── components/               # Reusable UI (Layout, DataTable, StatCard)
├── api/client.ts             # Axios client with all API endpoints
├── hooks/                    # Custom hooks (useWebSocket, etc.)
└── types/index.ts            # TypeScript interfaces
```

## Key Entry Points

- `rust-backend/src/main.rs` - Actix-web server setup, all API routes, middleware (CORS, Logger, Compress)
- `frontend/src/App.tsx` - React router, TanStack QueryClient config, protected routes with role-based access
- `frontend/src/api/client.ts` - All API client functions, WebSocket message types
- `rust-billing-engine/src/main.rs` - Billing engine initialization (dual-mode ESL)

## Database Schema (PostgreSQL)

Core tables in `apolo_billing` database:
- `accounts` - Customer accounts (prepaid/postpaid, balance, credit_limit, plan_id)
- `plans` - Account creation templates (initial_balance, credit_limit, max_concurrent_calls)
- `rate_cards` - Destination rates by prefix (LPM matching)
- `cdrs` - Call detail records with cost, duration, hangup cause
- `balance_reservations` - Active call balance holds
- `balance_transactions` - Recharges, consumptions, refunds
- `active_calls` - Currently ongoing calls
- `usuarios` - System users with roles (superadmin, admin, operator)
- `zonas`, `prefijos`, `tarifas` - Rate management hierarchy
- `audit_logs` - System audit trail

## Billing Flow

1. **CHANNEL_CREATE** → `AuthorizationService` validates account, finds rate via LPM, reserves initial balance
2. **CHANNEL_ANSWER** → `RealtimeBiller` starts periodic monitoring (every 180s), extends reservations as needed
3. **CHANNEL_HANGUP_COMPLETE** → `CdrGenerator` creates CDR, commits balance consumption, releases excess reservation

### Rate Matching (LPM)
Rates use Longest Prefix Match on `destination_prefix`. For destination `541156000`:
- Generates prefixes: ["5", "54", "541", "5411", "54115", "541156", ...]
- Queries rate_cards for matching prefix with valid dates
- Most specific (longest) prefix wins, ordered by priority

### Reservation Calculation
```
base = rate_per_minute × 5 minutes
buffer = base × 8%
total = clamp(base + buffer, $0.30, $30.00)
max_duration = (total / rate_per_minute) × 60 seconds
```

### Inbound vs Outbound Calls
- **Outbound** (context `from-pbx`): Account looked up by CALLER, charged
- **Inbound normal** (context `to-kamailio`): Skip authorization, no charge
- **Inbound toll-free** (0800/0801/1800): Account looked up by CALLEE (0800 owner pays)

## API Endpoints (Port 8000)

All routes under `/api/v1/`. Authentication via JWT in HTTP-only cookies.

| Endpoint | Method | Description | Access |
|----------|--------|-------------|--------|
| `/health` | GET | Health check | Public |
| `/auth/login` | POST | Login, sets JWT cookie | Public |
| `/auth/me` | GET | Current user info | Authenticated |
| `/auth/register` | POST | Register new user | Superadmin |
| `/auth/change-password` | POST | Change password | Authenticated |
| `/users/*` | CRUD | User management | Superadmin |
| `/audit-logs/*` | GET | Audit logs | Superadmin |
| `/accounts` | GET/POST | List/create accounts | Authenticated |
| `/accounts/{id}` | GET/PUT | Get/update account | Authenticated |
| `/accounts/{id}/topup` | POST | Add balance | Authenticated |
| `/plans/*` | CRUD | Plan management | Authenticated |
| `/rate-cards` | GET/POST | List/create rates | Authenticated |
| `/rate-cards/search/{phone}` | GET | LPM rate search | Authenticated |
| `/cdrs` | GET | List CDRs with pagination | Authenticated |
| `/cdrs/export` | GET | Streaming export (CSV/JSON) | Authenticated |
| `/cdrs/stats` | GET | CDR statistics | Authenticated |
| `/active-calls` | GET | List active calls | Authenticated |
| `/reservations/*` | CRUD | Balance reservations | Authenticated |
| `/stats` | GET | Dashboard statistics | Authenticated |
| `/dialplan/*` | CRUD | FreeSWITCH dialplan | Superadmin |
| `/rates/*` (zonas, prefijos, tarifas) | CRUD | Rate management hierarchy | Authenticated |
| `/ws` | WebSocket | Real-time updates | Authenticated |

## Billing Engine Modes

The billing engine operates in two modes based on `FREESWITCH_SERVERS` env var:
- **Testing mode** (empty/unset): Starts an ESL server on port 8021 that accepts connections from the call simulator
- **Production mode** (set to `host:port:password,...`): Connects as ESL client to FreeSWITCH server(s)

## Environment Variables

```bash
# Database
DATABASE_URL=postgresql://apolo_user:PASSWORD@localhost:5432/apolo_billing
DATABASE_MAX_CONNECTIONS=20
REDIS_URL=redis://localhost:6379

# Rust Backend
RUST_SERVER_HOST=0.0.0.0
RUST_SERVER_PORT=8000          # Must be 8000 for frontend proxy compatibility
RUST_SERVER_WORKERS=4
CORS_ORIGINS=http://localhost:3000,http://127.0.0.1:3000

# JWT Authentication
JWT_SECRET=your-secret-key
JWT_EXPIRATION_SECS=1800       # 30 minutes

# Billing Engine
ENVIRONMENT=development        # or production
HOST=0.0.0.0
PORT=9000
FREESWITCH_SERVERS=            # Empty = testing mode; "host:port:password" = production

# Logging
RUST_LOG=apolo_billing=info,apolo_api=info,actix_web=info
```

## Important Patterns

### PostgreSQL Timestamps
Always use `DateTime<Utc>` (not `NaiveDateTime`) for `TIMESTAMP WITH TIME ZONE` columns:
```rust
let created_at: DateTime<Utc> = row.get("created_at");
```

### Financial Calculations
Use `rust_decimal::Decimal` (not f64) for all monetary values across both backend and billing engine.

### API Response Pagination
Paginated endpoints return nested structure:
```json
{ "data": [...], "pagination": { "total": 100, "page": 1, "per_page": 20, "total_pages": 5 } }
```

### Role-Based Access
Three user roles with hierarchy: `superadmin` > `admin` > `operator`. Superadmin-only routes: users, audit-logs, dialplan, register.

### WebSocket Messages
Frontend receives real-time updates via `/ws` endpoint:
- `active_calls` - Full list of active calls
- `call_start`, `call_update`, `call_end` - Individual call events
- `stats_update` - Dashboard statistics

### Frontend State Management
TanStack Query v5 with: no refetch on window focus, 1 retry, 5s stale time, 60s cache for user data. All API calls go through `frontend/src/api/client.ts`.

## CI/CD

GitHub Actions workflows in `.github/workflows/`:
- **ci.yml** - On push/PR to main/develop: `cargo fmt`, `cargo clippy`, `cargo test`, `npm run lint`, `npm run build`
- **security.yml** - On push/PR + weekly: `cargo audit`, `npm audit`, CodeQL analysis
- **deploy.yml** - On push to main: SSH deploy, builds all components, restarts systemd services, health check on `/api/v1/health`

CI requires PostgreSQL 15 + Redis 7 services for Rust tests.

## Additional Documentation

- `rust-backend/crates/apolo-api/ARCHITECTURE.md` - CDR API architecture, performance targets, caching strategy
- `README_DEPLOYMENT.md` - Production deployment guide
- `rust-billing-engine/docs/CALL_SIMULATOR.md` - Testing with call simulator
