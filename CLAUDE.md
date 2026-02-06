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
- **frontend/** - React 19 + TypeScript + Vite SPA for dashboard UI
- **rust-backend/** - High-performance Rust API (Actix-web) for all CRUD operations, CDR queries, authentication
- **rust-billing-engine/** - Real-time billing processor via FreeSWITCH ESL

## Common Commands

### Rust Backend (Main API Server)
```bash
cd rust-backend
cargo build --release             # Build for production
cargo run --release               # Start server on :8000
cargo test                        # Run tests
cargo test <test_name>            # Run single test
```

### Frontend (React/TypeScript)
```bash
cd frontend
npm install                       # Install dependencies
npm run dev                       # Dev server on :3000 (proxies to :8000)
npm run build                     # Production build
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

### Systemd Services
```bash
# Backend API
systemctl status apolo-backend
journalctl -u apolo-backend -f

# Billing Engine
systemctl status apolo-billing-engine
journalctl -u apolo-billing-engine -f
systemctl restart apolo-billing-engine
```

## Key Entry Points

- `rust-backend/src/main.rs` - Actix-web server setup, all API routes
- `frontend/src/App.tsx` - React router and main app component
- `rust-billing-engine/src/main.rs` - Billing engine initialization

## Code Structure

### Rust Backend
```
rust-backend/
├── src/main.rs               # Server setup, route configuration
└── crates/
    ├── apolo-api/            # HTTP handlers and DTOs
    │   ├── handlers/         # Endpoint handlers (auth, account, cdr, rate_card, etc.)
    │   └── dto/              # Request/response types
    ├── apolo-db/             # PostgreSQL repositories
    ├── apolo-auth/           # JWT + Argon2 authentication
    ├── apolo-cache/          # Redis caching layer
    └── apolo-core/           # Shared models and traits
```

### Rust Billing Engine
```
rust-billing-engine/src/
├── services/                 # Core business logic
│   ├── authorization.rs      # Call authorization, LPM rate lookup
│   ├── realtime_biller.rs    # Active call monitoring, reservation extensions
│   ├── cdr_generator.rs      # CDR creation, cost calculation
│   └── reservation_manager.rs # Balance reservation CRUD
├── esl/                      # FreeSWITCH Event Socket Layer
│   └── event_handler.rs      # ESL event processing
├── api/                      # HTTP API routes
├── database/                 # PostgreSQL queries
└── cache.rs                  # Redis client
```

### Frontend
```
frontend/src/
├── pages/                    # Route components (Dashboard, CDR, Accounts, Rates, etc.)
├── components/               # Reusable UI (Layout, DataTable, StatCard)
├── api/client.ts             # Axios client, all API endpoints
├── hooks/                    # Custom hooks (useWebSocket, etc.)
└── types/index.ts            # TypeScript interfaces
```

## Database Schema (PostgreSQL)

Core tables in `apolo_billing` database:
- `accounts` - Customer accounts (prepaid/postpaid, balance, credit_limit)
- `rate_cards` - Destination rates by prefix (LPM matching)
- `cdrs` - Call detail records with cost, duration, hangup cause
- `balance_reservations` - Active call balance holds
- `balance_transactions` - Recharges, consumptions, refunds
- `active_calls` - Currently ongoing calls
- `usuarios` - System users
- `zonas`, `prefijos`, `tarifas` - Rate management hierarchy

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

## API Endpoints (Port 8000)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/auth/login` | POST | Login, returns JWT cookie |
| `/api/v1/auth/me` | GET | Current user info |
| `/api/v1/accounts` | GET/POST | List/create accounts |
| `/api/v1/accounts/{id}` | GET/PUT | Get/update account |
| `/api/v1/accounts/{id}/topup` | POST | Add balance |
| `/api/v1/rate-cards` | GET/POST | List/create rates |
| `/api/v1/rate-cards/search/{phone}` | GET | LPM search |
| `/api/v1/cdrs` | GET | List CDRs with pagination |
| `/api/v1/cdrs/export` | GET | Streaming export (CSV/JSON) |
| `/api/v1/active-calls` | GET | List active calls |
| `/api/v1/stats` | GET | Dashboard statistics |
| `/api/v1/zonas`, `/api/v1/prefijos`, `/api/v1/tarifas` | CRUD | Rate management |

## Environment Variables

```bash
# Database
DATABASE_URL=postgresql://apolo_user:PASSWORD@localhost:5432/apolo_billing
REDIS_URL=redis://localhost:6379

# Rust Backend
RUST_SERVER_HOST=0.0.0.0
RUST_SERVER_PORT=8000
CORS_ORIGINS=http://localhost:3000

# JWT Authentication
JWT_SECRET=your-secret-key
JWT_EXPIRATION_SECS=1800

# Billing Engine
ESL_HOST=127.0.0.1
ESL_PORT=8021

# Logging
RUST_LOG=apolo_billing=info,apolo_api=info,actix_web=info
```

## Important Patterns

### PostgreSQL Timestamps
Always use `DateTime<Utc>` (not `NaiveDateTime`) for `TIMESTAMP WITH TIME ZONE` columns:
```rust
let created_at: DateTime<Utc> = row.get("created_at");
```

### API Response Pagination
CDR endpoint returns nested pagination:
```json
{ "data": [...], "pagination": { "total": 100, "page": 1, "per_page": 20, "total_pages": 5 } }
```

### WebSocket Messages
Frontend receives real-time updates via WebSocket:
- `active_calls` - Full list of active calls
- `call_start`, `call_update`, `call_end` - Individual call events
- `stats_update` - Dashboard statistics

### Inbound vs Outbound Calls
- **Outbound** (context `from-pbx`): Account looked up by CALLER, charged
- **Inbound normal** (context `to-kamailio`): Skip authorization, no charge
- **Inbound toll-free** (0800/0801/1800): Account looked up by CALLEE (0800 owner pays)

## Testing

```bash
# Rust billing engine with ESL simulator
cd rust-billing-engine && cargo test

# Frontend linting
cd frontend && npm run lint

# Run specific test
cargo test test_authorization_flow
```

## Additional Documentation

- `README_DEPLOYMENT.md` - Production deployment guide
- `DEPLOYMENT_GUIDE_WSL.md` - WSL development setup
- `rust-backend/crates/apolo-api/ARCHITECTURE.md` - API architecture details
- `rust-billing-engine/docs/CALL_SIMULATOR.md` - Testing with call simulator
