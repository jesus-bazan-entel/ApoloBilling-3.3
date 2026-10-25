# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Apolo SBC** (repo still named ApoloBilling) is the management platform of a Kamailio + FreeSWITCH SBC: SIP devices and their customers, routing, CDRs, active calls and NOC monitoring.

> **No billing.** The billing module (rates, zones, plans, balances, reservations, call authorization, cost rating) was removed on 2026-10-24. CDRs are still recorded (no account, rate or cost). The billing tables are left in PostgreSQL unused (`rate_cards`, `zonas`, `prefijos`, `tarifas`, `plans`, `balance_reservations`, `balance_transactions`, billing columns of `accounts`/`cdrs`); drop them with a dedicated migration if ever needed. The `rust-billing-engine/` directory, the `apolo-billing*` binaries and systemd unit names keep their old names for deploy compatibility.

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
│  - Accounts CRUD + Topup      - Active Calls & Reservations     │
│  - Rate Cards CRUD + LPM      - Dashboard Stats                 │
│  - Zones/Prefixes/Tariffs     - Dialplan Management             │
│  - Plans / Users / Audit      - Unified Routing (In/Outbound)   │
│  - SIP Devices + Directory    - SIP OPTIONS Monitoring          │
│  - Kamailio Integration                                         │
└─────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
┌──────────────────┐  ┌──────────────┐  ┌──────────────────┐
│ SBC engine       │  │  PostgreSQL  │  │  MySQL/MariaDB   │
│    (:9000)       │  │   (:5432)    │  │    (:3306)       │
│ ESL, real-time   │  │ apolo_billing│  │   kamailio DB    │
└──────────────────┘  └──────────────┘  └──────────────────┘
                              │
                              ▼
                      ┌──────────────┐
                      │    Redis     │
                      │   (:6379)    │
                      └──────────────┘
```

**Three main components:**
- **frontend/** - React 19 + TypeScript + Vite 7 SPA with TanStack Query, Radix UI, Tailwind CSS 4
- **rust-backend/** - Actix-web 4 API server (Cargo workspace with 7 crates) for all CRUD, auth, CDR queries
- **rust-billing-engine/** - SBC engine via FreeSWITCH ESL: tracks `active_calls`, writes CDRs (+ PDD), imports direct Kamailio CDRs (dual-mode: ESL server for testing, ESL client for production). No Redis, no authorization

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
cargo run                         # Start the engine on :9000 (health only)
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
    │   ├── handlers/
    │   │   ├── auth.rs               # Login, logout, password change
    │   │   ├── account.rs            # Account CRUD + topup
    │   │   ├── cdr.rs                # CDR queries, export, stats
    │   │   ├── rate_card.rs          # Rate cards + LPM search
    │   │   ├── dialplan.rs           # FreeSWITCH dialplan
    │   │   ├── unified_routing.rs    # Trunks, inbound/outbound routes
    │   │   ├── sip_device.rs         # SIP devices CRUD
    │   │   ├── freeswitch_directory.rs # mod_xml_curl endpoint
    │   │   ├── kamailio_dialplan.rs  # Kamailio carriers/groups
    │   │   ├── kamailio_endpoints.rs # PBX endpoints (type=9)
    │   │   └── ...
    │   └── dto/
    │       ├── routing.rs            # Trunk, route DTOs
    │       ├── sip_device.rs         # SIP device DTOs
    │       └── ...
    ├── apolo-db/             # PostgreSQL repositories (SQLx 0.8)
    │   └── repositories/
    │       ├── sip_device_repo.rs    # SIP device queries
    │       └── ...
    ├── apolo-auth/           # JWT (HTTP-only cookies) + Argon2 authentication
    ├── apolo-cache/          # Redis caching layer
    ├── apolo-core/           # Shared models, traits, error handling, configuration
    │   └── models/
    │       ├── sip_device.rs         # SIP device model
    │       └── ...
    ├── apolo-services/       # Business logic services
    │   └── sip_device.rs             # SIP OPTIONS monitoring service
    └── apolo-esl/            # FreeSWITCH Event Socket Layer integration
```

### Rust Billing Engine (SBC engine)
```
rust-billing-engine/src/
├── services/
│   ├── cdr_generator.rs      # CDR insert at a-leg hangup (E.164 destination + customer, no rate/cost)
│   ├── numbering.rs          # Routed number (strip 31xx), Peru E.164, customer lookup keys
│   └── kamailio_cdr_sync.rs  # Direct-routed inbound CDRs from Kamailio acc_cdrs
├── esl/                      # FreeSWITCH Event Socket Layer
│   └── event_handler.rs      # active_calls tracking, CDRs, b-leg PDD
├── api/                      # HTTP API (health only)
└── database/                 # PostgreSQL (tokio-postgres + deadpool)
```

### Frontend
```
frontend/src/
├── pages/
│   ├── Dashboard.tsx         # NOC dashboard (traffic, quality, capacity)
│   ├── Accounts.tsx          # Clientes (customers; status gates REGISTER)
│   ├── CDR.tsx               # Call detail records
│   ├── ActiveCalls.tsx       # Live calls
│   ├── Provisioning.tsx      # Customer + SIP device in one step
│   ├── SipDevices.tsx        # SIP device management (NEW)
│   ├── UnifiedRouting.tsx    # Menu "Enrutamiento": trunks, inbound/outbound routes
│   ├── KamailioDialplan.tsx  # Kamailio carriers/groups (NEW)
│   ├── Dialplan.tsx          # FreeSWITCH dialplan
│   ├── Users.tsx             # User management
│   ├── AuditLogs.tsx         # Audit trail
│   └── Login.tsx             # Authentication
├── components/               # Reusable UI (Layout, DataTable, StatCard, Badge)
├── contexts/
│   └── ThemeContext.tsx      # Dark mode support
├── api/client.ts             # Axios client with all API endpoints
├── hooks/                    # Custom hooks (useWebSocket, etc.)
└── types/index.ts            # TypeScript interfaces
```

## Key Entry Points

- `rust-backend/src/main.rs` - Actix-web server setup, all API routes, middleware (CORS, Logger, Compress)
- `frontend/src/App.tsx` - React router, TanStack QueryClient config, protected routes with role-based access
- `frontend/src/api/client.ts` - All API client functions, WebSocket message types
- `rust-billing-engine/src/main.rs` - Billing engine initialization (dual-mode ESL)

### Key Handler Files

- `unified_routing.rs` - All routing CRUD (trunks, groups, inbound/outbound routes, SIP status)
- `sip_device.rs` - SIP device management
- `freeswitch_directory.rs` - Dynamic authentication endpoint for mod_xml_curl
- `kamailio_dialplan.rs` - Kamailio carrier and group management

## Database Schema (PostgreSQL)

Core tables in `apolo_billing` database:

### Customers & Calls
- `accounts` - Customers ("Clientes": number, name, status, COD_PED, location). `status` ≠ active blocks the customer's SIP REGISTER. Billing columns (balance, credit_limit, plan_id, account_type) are unused
- `cdrs` - Call detail records (duration, billsec, hangup cause, `pdd_ms`, `mos`, `rtp_loss_pct`); partitioned monthly. `caller_number`/`called_number` = as exchanged with the carrier (percent-decoded, `called_number` keeps Integratel's `31`+2-digit routing prefix); `destination_prefix` (API/CSV `destination`) = routed destination in E.164; `account_id` = customer whose `sip_devices.sip_username` is the routed destination (inbound) or caller (outbound), NULL for FS Legado customers (~94% of inbound) — migration 023
- `active_calls` - Currently ongoing calls
- `noc_events` - Dashboard chart marks (migration 019)

### Users & Audit
- `usuarios` - System users with roles (superadmin, admin, operator)
- `audit_logs` - System audit trail

### Unified Routing System
- `routing_trunks` - SIP trunks (public=Kamailio, private=FreeSWITCH)
- `routing_trunk_groups` - Trunk groups for failover
- `routing_trunk_group_members` - Trunk to group mapping with priority/weight
- `routing_outbound_routes` - Outbound routing rules (prefix → trunk group)
- `routing_inbound_routes` - Inbound routing rules (DID pattern → destination)
- `routing_sip_status_log` - SIP OPTIONS monitoring results

### SIP Devices
- `sip_devices` - SIP extensions/devices (username, password, codecs, account)
- `freeswitch_allowed_ips` - IP whitelist for directory endpoint

### System Configuration
- `system_settings` - Key-value system settings (sip_options_enabled, etc.)

## Call Recording Flow

The engine (`rust-billing-engine`) only records, it never authorizes or rejects a call:

1. **CHANNEL_CREATE** (a-leg) → row in `active_calls`
2. **CHANNEL_ANSWER** (a-leg) → `active_calls.answer_time` / `status = 'answered'`
3. **CHANNEL_HANGUP_COMPLETE** → b-legs: their first 180/183 gives the PDD of their a-leg, their `rtp_audio_in_*` stats its media quality; a-leg: row removed from `active_calls`, CDR written by `CdrGenerator` (fastest PDD, worst MOS/loss of all legs)

PDD caveat: the next hop is almost always FS Legado, which answers 180/183 within one 20 ms FreeSWITCH tick, so `pdd_ms` is ~0–20 ms (PDD to the next hop, not the destination network's). Not shown on the dashboard since 2026-10-25 (see *NOC quality indicators*).
4. Calls routed Kamailio → trunk directly (no FreeSWITCH) get their CDRs from `kamailio_cdr_sync.rs`

Direction comes from FreeSWITCH's native headers (`Call-Direction`…), with the context as fallback.

### NOC quality indicators

Removed from the dashboard on 2026-10-25 because none of them measured anything real here (the
engine still writes `cdrs.pdd_ms`/`mos`/`rtp_loss_pct`):
- **MOS / jitter / loss from RTPEngine**: kernel mode (`table = 0`) forwards RTCP without parsing it, so its counters never move.
- **MOS from FreeSWITCH** (`rtp_audio_in_mos`): `R = int((recved − flaws)/recved·100)`, `MOS = 1 + 0.035R + 7e-6·R(R−60)(100−R)` (`switch_rtp.c` `do_mos`). With no sequence gaps it's exactly 4.50 (88% of calls); it only drops below 3.6 at ~30% flawed packets. No codec, delay or jitter term.
- **"Loss" = `in_skip_packet_count`**: counts reads where FreeSWITCH found no packet and generated CNG (silence, timer phase), not network loss; it tripled with mod_timerfd. Real seq-gap loss would be `in_flaw_total`.
- **PDD**: INVITE → next hop's first 180/183, quantised to FreeSWITCH's 20 ms clock (values 0/19/20 ms); the next hop (FS Legado) rings back instantly, so the destination network's PDD isn't visible from this SBC.

The quality panel keeps *Audio en un sentido* (RTPEngine per-session packet counters) and *CPS pico*.

## API Endpoints (Port 8000)

All routes under `/api/v1/`. Authentication via JWT in HTTP-only cookies.

### Core Endpoints

| Endpoint | Method | Description | Access |
|----------|--------|-------------|--------|
| `/health` | GET | Health check | Public |
| `/auth/login` | POST | Login, sets JWT cookie | Public |
| `/auth/me` | GET | Current user info | Authenticated |
| `/auth/register` | POST | Register new user | Superadmin |
| `/auth/change-password` | POST | Change password | Authenticated |
| `/users/*` | CRUD | User management | Superadmin |
| `/audit-logs/*` | GET | Audit logs | Superadmin |
| `/accounts` | GET/POST | List/create customers (Clientes) | Authenticated |
| `/accounts/{id}` | GET/PUT/DELETE | Get/update (phone, status)/delete customer | Authenticated |
| `/accounts/stats` | GET | Total / active customers | Authenticated |
| `/provisioning` (`/bulk`) | POST | Customer + SIP device (individual / bulk) | Superadmin |
| `/cdrs` | GET | List CDRs with pagination; dates are RFC 3339 with offset or `YYYY-MM-DD` (Lima day), anything else is 400 | Authenticated |
| `/cdrs/export` | GET | Streaming export (CSV/JSON) in 1-hour slices (no OFFSET; a month ≈ 5 s); `start_date` required, ≤31 days, default limit 1M, CSV times in Lima, RFC 4180 + formula guard (bare E.164 left intact), audited | Authenticated |
| `/cdrs/stats` | GET | CDR statistics | Authenticated |
| `/active-calls` | GET | List active calls | Authenticated |
| `/stats/noc` | GET | NOC dashboard overview | Authenticated |
| `/stats/noc/events` | POST/DELETE | Chart marks | Admin |
| `/dialplan/*` | CRUD | FreeSWITCH dialplan | Superadmin |
| `/settings` | GET/PUT | System settings | Superadmin |
| `/ws` | WebSocket | Real-time updates | Authenticated |

### Unified Routing Endpoints

| Endpoint | Method | Description | Access |
|----------|--------|-------------|--------|
| `/routing/trunks` | GET/POST | List/create trunks | Admin |
| `/routing/trunks/{id}` | GET/PUT/DELETE | CRUD individual trunk | Admin |
| `/routing/trunk-groups` | GET/POST | Trunk groups for failover | Admin |
| `/routing/trunk-groups/{id}` | GET/PUT/DELETE | CRUD trunk group | Admin |
| `/routing/trunk-groups/{id}/members` | GET/POST/DELETE | Group members | Admin |
| `/routing/outbound-routes` | GET/POST | Outbound routing rules | Admin |
| `/routing/outbound-routes/{id}` | GET/PUT/DELETE | CRUD outbound route | Admin |
| `/routing/inbound-routes` | GET/POST | Inbound routing rules | Admin |
| `/routing/inbound-routes/{id}` | GET/PUT/DELETE | CRUD inbound route | Admin |
| `/routing/sip-status` | GET | SIP OPTIONS peer status | Admin |
| `/routing/sync-kamailio` | POST | Sync trunks to Kamailio | Admin |

### SIP Devices Endpoints

| Endpoint | Method | Description | Access |
|----------|--------|-------------|--------|
| `/sip-devices` | GET/POST | List/create SIP devices | Admin |
| `/sip-devices/{id}` | GET/PUT/DELETE | CRUD individual device | Admin |
| `/sip-devices/{id}/status` | GET | Device registration status | Admin |
| `/freeswitch/directory` | POST | Dynamic auth (mod_xml_curl) | IP Whitelist |
| `/freeswitch/allowed-ips` | GET/POST | Manage IP whitelist | Superadmin |

### Kamailio Integration Endpoints

| Endpoint | Method | Description | Access |
|----------|--------|-------------|--------|
| `/kamailio-dialplan/groups` | GET/POST | Carrier groups (dr_gw_lists) | Superadmin |
| `/kamailio-dialplan/carriers` | GET/POST | Carriers (dr_gateways type=8) | Superadmin |
| `/kamailio-dialplan/routes` | GET/POST | Routing rules (dr_rules) | Superadmin |
| `/kamailio-endpoints` | GET/POST | PBX endpoints (type=9) | Superadmin |

## Engine Modes

The engine (`rust-billing-engine`) operates in two modes based on `FREESWITCH_SERVERS` env var:
- **Testing mode** (empty/unset): Starts an ESL server on port 8021 that accepts connections from the call simulator
- **Production mode** (set to `host:port:password,...`): Connects as ESL client to FreeSWITCH server(s)

## Environment Variables

```bash
# Database (PostgreSQL - Main)
DATABASE_URL=postgresql://apolo_user:PASSWORD@localhost:5432/apolo_billing
DATABASE_MAX_CONNECTIONS=20

# Database (MySQL - Kamailio, optional)
KAMAILIO_DATABASE_URL=mysql://kamailio:PASSWORD@localhost:3306/kamailio

# Redis
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

### System Settings (stored in database)

These settings are stored in the `system_settings` table and can be modified via API:

| Key | Default | Description |
|-----|---------|-------------|
| `sip_options_enabled` | `false` | Enable SIP OPTIONS health monitoring |
| `sip_options_interval` | `30` | Interval (seconds) between SIP OPTIONS checks |

## Important Patterns

### PostgreSQL Timestamps
Always use `DateTime<Utc>` (not `NaiveDateTime`) for `TIMESTAMP WITH TIME ZONE` columns:
```rust
let created_at: DateTime<Utc> = row.get("created_at");
```

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

## Database Migrations

Migrations are located in `database/migrations/` and should be run in order:

| Migration | Description |
|-----------|-------------|
| 002_add_plans_table.sql | Account plans table |
| 003_add_sip_devices.sql | SIP devices and base tables |
| 003_add_missing_tables.sql | Additional missing tables |
| 004_unified_routing.sql | Unified routing system (trunks, groups, routes) |
| 005_trunk_type_and_sip_status.sql | Trunk types (public/private) and SIP OPTIONS logging |
| 006_internal_routes.sql | Internal routes and system endpoints (dropped by 022) |
| 007_inbound_route_transformation.sql | destination_transform field |
| 008_inbound_route_trunk_destination.sql | trunk_destination_id field |
| 009_inbound_early_media.sql | early_media field for inbound routes |
| 010_inbound_route_kamailio_ruleid.sql | kamailio_ruleid field for inbound routes |
| 011_outbound_route_prefix.sql | strip_digits / prefix_to_add for outbound routes (per-route number transformation) |
| 012_inbound_route_parallel_ring.sql | ring_registered_parallel boolean (superseded by 013, column left orphaned) |
| 013_inbound_route_parallel_trunk.sql | parallel_trunk_id — ring a second trunk in parallel with the inbound destination (simultaneous fork, first answer wins) |
| 014_account_provisioning_fields.sql | cod_ped / departamento / provincia / distrito on accounts + activates the previously-unused account_name (CLIENTE); used by the Provisioning module |
| 015_drop_duplicate_cdr_indexes.sql | Drops idx_cdr_account/caller/start_time/uuid — exact duplicates of idx_cdrs_account_id/caller/start_time/uuid on the same table/columns |
| 016_partition_cdrs.sql | Monthly range partitioning of cdrs on start_time |
| 017_inbound_direct_routing.sql | `direct_routing` / `fs_numbers` / `kamailio_fs_ruleid` on inbound routes + `kamailio_cdr_sync_state` cursor (Kamailio → trunk without FreeSWITCH; see *Direct inbound routing*) |
| 018_monitoring_trunk_health.sql | `trunks_online` / `trunks_total` on `monitoring_kamailio_metrics` for the *Trunk Health* card (SIP OPTIONS via Kamailio) |
| 019_noc_events.sql | `noc_events` — incident/campaign/migration marks on the dashboard traffic chart (`POST/DELETE /stats/noc/events`, admin+). Dashboard thresholds/capacity are optional `system_settings` keys: `noc_asr_warn/crit`, `noc_ner_warn/crit`, `noc_min_range_attempts`, `noc_max_channels`, `noc_max_cps` |
| 020_noc_live_quality_and_pdd.sql | `cdrs.pdd_ms` (INVITE → first 180/183, written by the billing engine from FS `*_uepoch` vars; NULL on direct Kamailio CDRs) + SELECT on `monitoring_rtpengine_metrics`/`monitoring_kamailio_metrics` for the backend role (dashboard quality = last 15 min of 1-min samples) |
| 021_cdr_media_quality.sql | `cdrs.mos` / `cdrs.rtp_loss_pct`: worst leg's FreeSWITCH-measured MOS and received-RTP loss (`rtp_audio_in_mos`, `skip_packet_count / media_packet_count`), written by the engine. Still written, but no longer on the dashboard (see *NOC quality indicators*) |
| 022_drop_internal_routes.sql | Drops `internal_routes` / `system_endpoints` (the *Rutas Internas* module, removed 2026-10-25; backup in `/root/internal_routes-backup-*.sql`) |
| 023_cdr_destination_e164_account.sql | `cdrs.destination_prefix` repurposed as the routed destination in E.164 + `account_id` from `sip_devices` (SQL mirrors `apolo_routed_number`/`apolo_e164` of the engine's `numbering.rs`); backfills October 2026 (`SET statement_timeout = 0`, ~1 min) |

Kamailio (MySQL) schema additions live in `database/kamailio/*.sql` (applied by `install_server.sh` → `configure_kamailio`).

Run migrations:
```bash
for f in database/migrations/*.sql; do
    psql -U apolo_user -d apolo_billing -f "$f"
done
```

## Additional Documentation

- `rust-backend/crates/apolo-api/ARCHITECTURE.md` - CDR API architecture, performance targets, caching strategy
- `README_DEPLOYMENT.md` - Production deployment guide
- `rust-billing-engine/docs/CALL_SIMULATOR.md` - Testing with call simulator
- `INSTALL.md` - Complete installation guide for Debian 12 (PostgreSQL, MySQL, Kamailio, FreeSWITCH, RTPEngine, Nginx)
- `docs/RTP_FLOW_ARCHITECTURE.md` - **RTP flow architecture, bypass analysis, MOH/DTMF/early media configurations**

---

## Kamailio Integration (dSIPRouter)

ApoloBilling integrates with Kamailio/dSIPRouter for SIP routing and carrier management.

### Kamailio Database (MySQL)

Optional connection via `KAMAILIO_DATABASE_URL` environment variable. Tables managed:

| Table | Purpose |
|-------|---------|
| `dr_gateways` | SIP gateways (type=8 carriers, type=9 PBX) |
| `dr_gw_lists` | Gateway groups for failover |
| `dr_rules` | Outbound routing rules (prefix matching) |
| `address` | IP-based ACL (trusted peers) |
| `dsip_gw2gwgroup` | Gateway to group mapping |
| `dsip_call_settings` | Call limits per group |

### Gateway Types

- **type=8**: Carrier/trunk (external providers like VOIPSWITCH)
- **type=9**: PBX endpoint (FreeSWITCH internal)

### Kamailio Reload Commands

```bash
kamcmd drouting.reload           # Reload gateways and routes
kamcmd permissions.addressReload # Reload IP ACL
kamcmd htable.reload gw2gwgroup  # Reload gateway mappings
```

### Kamailio API Endpoints

| Endpoint | Description |
|----------|-------------|
| `/api/v1/kamailio-dialplan/groups` | Carrier group management |
| `/api/v1/kamailio-dialplan/carriers` | Individual carrier CRUD |
| `/api/v1/kamailio-dialplan/routes` | Outbound routing rules |
| `/api/v1/kamailio-endpoints` | PBX endpoint management (type=9) |

---

## FreeSWITCH ↔ Kamailio Architecture

### Network Topology

```
                              INTERNET / PSTN
                                    │
                                    ▼
┌───────────────────────────────────────────────────────────────────────────────┐
│                           SERVIDOR (ej: 10.10.22.4)                           │
│                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────┐  │
│  │                      KAMAILIO / dSIPRouter                              │  │
│  │                           :5060 (SBC)                                   │  │
│  │                                                                         │  │
│  │   Carriers externos (type=8):                                           │  │
│  │   - VOIPSWITCH_IN  (setid=13) ← recibe de 190.105.250.x                │  │
│  │   - VOIPSWITCH_OUT (setid=14) → envía a 172.16.1.25                    │  │
│  │                                                                         │  │
│  │   PBX interno (type=9):                                                 │  │
│  │   - FreeSWITCH (gwid=100) → 127.0.0.1:5080                             │  │
│  │                                                                         │  │
│  └────────────────────────────┬────────────────────────────────────────────┘  │
│                               │ SIP (UDP)                                     │
│  ┌────────────────────────────▼────────────────────────────────────────────┐  │
│  │                         FREESWITCH                                      │  │
│  │                                                                         │  │
│  │   Internal Profile (:5080)     External Profile (:5062)                 │  │
│  │   - contexto: from-pbx         - contexto: public                       │  │
│  │   - Llamadas de Kamailio       - Llamadas a Kamailio (troncales)        │  │
│  │   - Dispositivos SIP           - Gateway kamailio configurado           │  │
│  │                                                                         │  │
│  │   mod_xml_curl → http://127.0.0.1:8000/api/v1/freeswitch/directory     │  │
│  │   (autenticación dinámica de dispositivos SIP)                          │  │
│  │                                                                         │  │
│  └─────────────────────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────────────────────┘
```

### Call Flow: Inbound (PSTN → Extension)

```
PSTN → VOIPSWITCH (190.105.250.x) → Kamailio :5060 → FreeSWITCH :5080 → Extension
```

1. Carrier sends INVITE to Kamailio
2. Kamailio looks up `dr_gateways` (type=9, gwid=100)
3. Routes to FreeSWITCH internal profile (:5080)
4. FreeSWITCH uses `from-pbx` context, finds extension

### Call Flow: Outbound (Extension → PSTN)

```
Extension → FreeSWITCH :5080 → Kamailio :5060 → VOIPSWITCH (172.16.1.25) → PSTN
```

1. Extension calls via FreeSWITCH
2. Dialplan bridges to Kamailio gateway
3. Kamailio applies drouting rules
4. Selects carrier from `dr_gw_lists` (setid=14)

### FreeSWITCH Directory Endpoint

`POST /api/v1/freeswitch/directory` - Dynamic SIP user authentication

- IP whitelist security (no JWT required)
- Returns XML with A1 hash, codecs, caller ID
- Used by mod_xml_curl during REGISTER

### FreeSWITCH ↔ Kamailio links (PostgreSQL)

The interconnection is defined by private trunks (each auto-provisions a type=9 `dr_gateways` row),
`dr_rules` and the generated `from-kamailio.xml`. The old *Rutas Internas* module
(`internal_routes` / `system_endpoints`, a sync stub that never wrote anything) was removed on
2026-10-25 (migration 022).

- `routing_trunks` - Unified trunk management (private/public)
- `routing_sip_status_log` - SIP OPTIONS monitoring

### Trunk Types

- **private** (trunk_type='private'): Internal FreeSWITCH trunks
- **public** (trunk_type='public'): External carriers via Kamailio

---

## Unified Routing System

The unified routing system provides centralized management for all inbound and outbound call routing.

### Trunks

Trunks represent SIP connections to carriers or internal systems:

```rust
// Trunk types
enum TrunkType {
    Public,   // External carrier via Kamailio (syncs to dr_gateways)
    Private,  // Internal FreeSWITCH trunk
}

// Key fields
struct Trunk {
    name: String,
    trunk_type: TrunkType,
    host: String,
    port: u16,
    username: Option<String>,
    password: Option<String>,
    transport: String,  // udp, tcp, tls
    codecs: String,
    enabled: bool,
}
```

### Trunk Groups

Groups provide failover capability with priority and weight distribution:

```sql
-- Example: Create trunk group with 2 carriers
INSERT INTO routing_trunk_groups (name, description) VALUES ('Primary Carriers', '...');
INSERT INTO routing_trunk_group_members (trunk_group_id, trunk_id, priority, weight)
VALUES (1, 1, 10, 100), (1, 2, 20, 100);  -- Trunk 1 has higher priority
```

### Outbound Routes

Outbound routes match by prefix and route to trunk groups:

```sql
-- Example: Route Argentina mobile to primary carriers
INSERT INTO routing_outbound_routes (name, prefix, trunk_group_id, priority, enabled)
VALUES ('Argentina Mobile', '5411', 1, 10, true);
```

### Inbound Routes

Inbound routes match by DID pattern and route to destinations:

```sql
-- Example: Route DID to extension with number transformation
INSERT INTO routing_inbound_routes (
    name, did_pattern, destination, destination_type,
    destination_transform, early_media, enabled
)
VALUES (
    'Main DID', '541155550XXX', '100', 'extension',
    '', false, true
);
```

**Pattern matching:**
- `X` = any digit 0-9
- `N` = any digit 2-9
- `.` = any number of digits
- `[abc]` = any of a, b, c

**Number transformation (outbound + inbound):** `strip_digits` removes N leading digits and
`prefix_to_add` prepends a string. On inbound routes these are applied in the generated
`from-kamailio.xml` (`${transformed_number}`); on outbound routes they map to a dedicated
Kamailio gateway's `dr_gateways.strip`/`pri_prefix` (so they take effect AFTER drouting).

**Direct inbound routing (`direct_routing`, migration 017):** by default every inbound DID goes
Carrier → Kamailio → FreeSWITCH-SBC → trunk. With `direct_routing=true` the route's `dr_rules` row
points at a dedicated type=9 gateway on `destination_trunk_id` carrying the route's
`strip_digits`/`prefix_to_add` and tagged `direct` in `dr_gateways.attrs` field 6, so Kamailio
delivers straight to the trunk (e.g. FS Legado). App users are diverted automatically: the backend mirrors every enabled
`sip_devices.sip_username` of an active customer (`route_id = 'sip_devices'`, on device
create/update/delete, provisioning and every 60s — `sync_app_fs_numbers`), plus any number a
route lists by hand in `fs_numbers` (post-transform). Those numbers are diverted in `kamailio.cfg` NEXTHOP via htable `inbound_fs_numbers`
(table `kamailio.apolo_inbound_fs_numbers`, reloaded every 30s) back to FreeSWITCH through dr group
**9100** (`kamailio_fs_ruleid`), keeping the parallel ring. Calls that skip FreeSWITCH have no ESL
events: their CDRs come from Kamailio dialog CDRs (`acc` `cdr_enable`, table `acc_cdrs`, `via=direct`)
imported by `rust-billing-engine/src/services/kamailio_cdr_sync.rs` (needs `KAMAILIO_DATABASE_URL`).
`via=fs` rows are kept only for `scripts/validate_kamailio_cdrs.sh`.
**Status (2026-10-25):** no route is direct — all Integratel inbound goes through FreeSWITCH-SBC
(decision: simpler to operate). The canary (`dr_rules` 2071, `3157` → direct gw 960, live
2026-10-24 02:44 → 2026-10-25 13:03) was removed; backup in `/root/dr_rule_2071_canary-*.sql`.
The code stays: ticking *Enrutar directo* on a route re-enables it, and the app-number mirror
keeps running so `fs_numbers` is ready. Gw 960 and divert rule 2070 (group 9100) are inert.

**Trunk health (migration 018):** the monitoring collector (`backend/app/monitoring/service.py`,
`KamailioCollector._probe_trunks`) sends SIP OPTIONS every 60s to each enabled `routing_trunks` row
through Kamailio's own sockets (`kamcmd tm.t_uac_wait_block`: public trunks from the carrier-facing
listener, private from the LAN one), updates `routing_trunks.sip_status`/`last_options_*`, logs to
`routing_sip_status_log` (7-day retention) and stores `trunks_online/total` for the dashboard's
*Trunk Health* card, which replaced *Dispatcher Health* (the dispatcher table is empty since 2026-10-24).

**Parallel ring (`parallel_trunk_id`):** an inbound route can ring a second destination at the
same time as the primary (first to answer wins). Selecting a trunk (e.g. an SBC Push) makes the
generated dialplan `bridge <primary>,${sofia_contact(<num>@${local_ip_v4})}` — the parallel leg
targets the number's **registered contact** (which reaches a mobile app via the push SBC's
path/FCM), NOT a raw INVITE to the SBC IP (that loops → 482). **The bridge MUST set
`ignore_early_media=true`**, otherwise the first leg's 183-with-SDP makes FreeSWITCH cancel the
slower (push) leg before the app rings. Inbound routing also requires Kamailio to send the call
to FreeSWITCH first (not straight to the destination trunk) so FS can transform + fork — see
`resolve_inbound_target_gwid` (internal-profile routes ignore `destination_trunk_id` as the
Kamailio next-hop).

### SIP OPTIONS Monitoring

The system can monitor trunk health via SIP OPTIONS:

```rust
// Status values
enum SipStatus {
    Online,   // 200 OK received
    Offline,  // No response or error
    Unknown,  // Not yet checked
}

// Results stored in routing_sip_status_log
struct SipStatusLog {
    trunk_id: i32,
    sip_status: SipStatus,
    last_check: DateTime<Utc>,
    response_time_ms: Option<i32>,
    error_message: Option<String>,
}
```

### Kamailio Synchronization

Public trunks are automatically synchronized to Kamailio tables:

1. `routing_trunks` → `dr_gateways` (type=8)
2. `routing_trunk_groups` → `dr_gw_lists`
3. `routing_outbound_routes` → `dr_rules`

Use `POST /api/v1/routing/sync-kamailio` to trigger manual sync.

---

## SIP Devices

SIP devices are managed in PostgreSQL and authenticated dynamically via mod_xml_curl.

### Accounts ↔ SIP Devices relationship

`sip_devices.account_id` → `accounts.id` (many-to-one, **NOT NULL**): a device always
belongs to one billing account; an account may have several devices. In Fonowin
provisioning the convention is **1:1**, with `accounts.account_number = sip_devices.sip_username`.

```
        accounts (pays)                       sip_devices (registers)
        ┌────────────────┐   account_id FK    ┌─────────────────────────┐
        │ id ◄───────────┼────────────────────┤ account_id (NOT NULL)   │
        │ account_number │   1            N    │ sip_username            │
        │ status         │                     │ a1_hash, sip_domain     │
        │ balance / plan │                     │ enabled                 │
        └────────────────┘                     └─────────────────────────┘
```

- **`sip_devices` authenticates** the REGISTER (sip_username + a1_hash + `enabled`).
- **`accounts` authorizes** it (the linked account must be `status = 'active'`) and is what
  gets billed (via `accountcode`, or `account_id` when `accountcode` is empty).

### Device Registration Flow

The softphone registers against FreeSWITCH's internal profile (:5080). FreeSWITCH has no
static user files — it resolves users dynamically through `mod_xml_curl`, which queries the
backend. The backend answers from the `sip_devices` + `accounts` tables.

```
 Softphone            FreeSWITCH (:5080)        Backend /freeswitch/directory      PostgreSQL
    │                       │                              │                            │
    │  REGISTER (no auth)   │                              │                            │
    │──────────────────────▶│                              │                            │
    │   401 + nonce         │                              │                            │
    │◀──────────────────────│                              │                            │
    │  REGISTER + digest    │                              │                            │
    │  (user, realm, resp)  │                              │                            │
    │──────────────────────▶│                              │                            │
    │                       │  mod_xml_curl POST           │                            │
    │                       │  (sip_auth_username, realm)  │                            │
    │                       │─────────────────────────────▶│                            │
    │                       │                              │ 1. IP in freeswitch_       │
    │                       │                              │    allowed_ips? (no JWT)   │
    │                       │                              │ 2. find_by_username_domain │
    │                       │                              │    INNER JOIN accounts     │
    │                       │                              │    ON status='active'      │
    │                       │                              │───────────────────────────▶│
    │                       │                              │   device row (or none)     │
    │                       │                              │◀───────────────────────────│
    │                       │                              │ 3. enabled? account active?│
    │                       │   directory XML              │                            │
    │                       │   <param a1-hash=...>        │                            │
    │                       │   <variable accountcode=...> │                            │
    │                       │◀─────────────────────────────│                            │
    │                       │ 4. validate digest response  │                            │
    │                       │    against a1-hash (MD5)      │                            │
    │   200 OK / 403        │                              │                            │
    │◀──────────────────────│                              │                            │
```

**Steps & validations** (`handlers/freeswitch_directory.rs`):

1. **IP whitelist** — the endpoint uses no JWT; it only trusts source IPs listed in
   `freeswitch_allowed_ips`.
2. **Device lookup** — `find_by_username_domain(sip_username, realm)`. The repo query
   **INNER JOINs `accounts` with `status = 'active'`**, so a device whose account is
   suspended/closed simply isn't found.
3. **Three blockers** (any one returns `<result status="not found"/>` ⇒ REGISTER fails):
   device does not exist · device `enabled = false` · linked account not `active`.
4. **Success XML** returns `a1-hash` (= `sip_devices.a1_hash`) and `accountcode`
   (explicit, or `account_id` as fallback — this is what links the CDR to the billed
   account). FreeSWITCH then validates the REGISTER digest against that `a1-hash`.

> ⚠️ Never set `force-register-domain` in `internal.xml`: it rewrites the REGISTER realm
> before `mod_xml_curl` runs, so the recomputed digest no longer matches the `a1_hash`
> (which was generated with the device's real domain) → silent 403 on every device.

### Device Model

```rust
struct SipDevice {
    username: String,           // Extension number (e.g., "1001")
    password: String,           // Stored as A1 hash (MD5)
    domain: String,             // SIP domain
    display_name: String,       // Caller ID name
    account_id: Option<i32>,    // Linked billing account
    enabled: bool,
    codecs: String,             // "PCMA,PCMU,G729"
    max_calls: i32,             // Max concurrent calls
    nat_handling: bool,
    record_calls: bool,
}
```

### A1 Hash Calculation

```rust
// A1 hash for SIP digest authentication
fn calculate_a1_hash(username: &str, realm: &str, password: &str) -> String {
    let input = format!("{}:{}:{}", username, realm, password);
    format!("{:x}", md5::compute(input))
}
```

---

## Production Ports Summary

| Port | Service | Protocol | Description |
|------|---------|----------|-------------|
| 80/443 | Nginx | TCP | Web frontend + API proxy |
| 5060 | Kamailio | UDP/TCP | SIP signaling (SBC) |
| 5080 | FreeSWITCH | UDP/TCP | Internal SIP profile |
| 5062 | FreeSWITCH | UDP/TCP | External SIP profile |
| 8000 | Rust Backend | TCP | REST API |
| 9000 | SBC engine | TCP | Health endpoint only (CDRs/active calls via ESL) |
| 8021 | FreeSWITCH ESL | TCP | Event Socket Layer |
| 5432 | PostgreSQL | TCP | Main database |
| 3306 | MySQL | TCP | Kamailio database |
| 6379 | Redis | TCP | Cache |
| 10000-20000 | RTPEngine | UDP | Media relay |

## Session Memory

### Frontend
- Puerto 3000 (dev en Vite), 80/443 (prod con Nginx)
- **Reinicio**: `sudo systemctl restart apolo-frontend`
- Nginx config: `deploy/nginx/apolo-frontend.conf` (único que se usa)
- La API devuelve paginación anidada: `{ data: [...], pagination: { total_pages, page, per_page, total } }`
- `fetchAccounts` debe iterar todas las páginas para tener la lista completa de cuentas
- Columna "Cuenta" en SIP Devices: muestra `account.account_number`, fallback `ID: {account_id}` si no se encuentra la cuenta

### Base de Datos
- **MySQL** (`KAMAILIO_DATABASE_URL`): solo para integración con Kamailio (tablas `dr_gateways`, `dr_rules`, `dr_gw_lists`, `address`, `dsip_*`). Opcional.
- PostgreSQL: 3165 cuentas totales en el entorno actual

### Trunks (3 en producción)
| ID | Nombre | Tipo | Host:Puerto |
|----|--------|------|-------------|
| `642dfd41-...` | SBC Push | private | 10.118.0.5:5060 |
| `ba4d355d-...` | Freeswitch Legado | private | 10.118.0.4:5060 |
| `ca36716b-...` | Integratel | public | 10.35.12.68:5060 |

### Enrutamiento Saliente (`dr_rules` groupid=8000)
- **Integratel** maneja prefijos: `2031` (Entel), `2131` (Claro), `2231` (Movistar), `2431` (Bitel), `3231` (TDP Fijo), `4631` (GTD), `5731` (ON)
- **Freeswitch Legado** maneja prefijos: `0800`, `0801`, `04`, `05`, `06`, `07`, `08`, `2`, `3`, `4`, `5`, `6`, `7`, `9` (con transformaciones de prefijo)
- **Prefijo `1`**: gwid=953 (type=9, FS external `app.fonowin.pe:5062`) — rango DID Fonowin

### Números Cortos (ej: "123")
- No hay manejo especial para números cortos
- `123` matchea prefijo `1` → gwid=953 → FreeSWITCH external profile → NO va a Integratel
- No existen rutas con prefijo `123` ni `12` en `dr_rules`
- Si no existe el DID en FreeSWITCH, la llamada termina en 404

### Provisioning
- Endpoints: `POST /api/v1/provisioning` (individual), `POST /api/v1/provisioning/bulk` (masivo)
- Crea cuenta + dispositivo SIP atómicamente (rollback si falla el device)
- Bulk: acepta `.xlsx` y `.csv`, parseo sin librerías externas, máx 5000 filas
