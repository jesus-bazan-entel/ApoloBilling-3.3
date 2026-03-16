# apolo-db

Database layer for ApoloBilling with PostgreSQL repository implementations.

## Overview

This crate provides:

- **Connection Pool Management**: PostgreSQL connection pooling with sqlx
- **Repository Implementations**: Concrete implementations of all repository traits from apolo-core
- **Optimized Queries**: Efficient database access with prepared statements and indexing
- **Longest Prefix Match (LPM)**: Fast rate lookups using PostgreSQL array operations

## Repositories

### PgAccountRepository
- CRUD operations for accounts
- `find_by_number`: Lookup by account number with phone normalization
- `find_by_phone`: ANI-based account lookup
- `update_balance`: Atomic balance updates
- `list_filtered`: Paginated listing with status/type filters

### PgRateRepository
- CRUD operations for rate cards
- `find_by_destination`: **LPM algorithm** using PostgreSQL `ANY($1)` with prefix array
- `search`: Flexible search by prefix pattern and name
- `bulk_create`: Efficient bulk insertion with transactions

### PgCdrRepository
- CRUD operations for call detail records
- `find_by_uuid`: Fast UUID-based lookup
- `list_filtered`: Advanced filtering by account, caller, callee, and date range

### PgUserRepository
- CRUD operations for users
- `find_by_username`: Authentication lookup
- `find_by_email`: Email-based user search
- `update_last_login`: Track login times

### PgReservationRepository
- CRUD operations for balance reservations
- `find_by_call_uuid`: Get reservation for active call
- `find_active_by_account`: List active reservations
- `count_active_by_account`: Count concurrent calls
- `update_status`: Update reservation lifecycle
- `expire_old`: Background job to expire stale reservations

## Database Schema

The repositories expect the following PostgreSQL schema:

### Tables
- `accounts`: Customer accounts with balance tracking
- `rate_cards`: Destination rates with prefix matching
- `cdrs`: Call detail records
- `usuarios`: System users
- `balance_reservations`: Active balance holds

See `/opt/ApoloBilling/schema.sql` for full schema definition.

## Usage

```rust
use apolo_db::{create_pool, PgAccountRepository, PgRateRepository};
use apolo_core::traits::{AccountRepository, RateRepository};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create connection pool
    let pool = create_pool("postgresql://localhost/apolo_billing", None).await?;

    // Initialize repositories
    let account_repo = PgAccountRepository::new(pool.clone());
    let rate_repo = PgRateRepository::new(pool.clone());

    // Use repositories
    let account = account_repo.find_by_number("100001").await?;
    let rate = rate_repo.find_by_destination("51999888777").await?;

    Ok(())
}
```

## Compilation Note

This crate uses sqlx macros (`query_as!`, `query!`) which require compile-time database verification.

### Option 1: Use sqlx-cli to prepare queries offline
```bash
cd /opt/ApoloBilling/rust-backend
DATABASE_URL=postgresql://user:pass@localhost/apolo_billing cargo sqlx prepare
```

### Option 2: Set DATABASE_URL during compilation
```bash
DATABASE_URL=postgresql://user:pass@localhost/apolo_billing cargo build -p apolo-db
```

### Option 3: Use runtime queries (fallback)
If you don't have database access during compilation, the code can be modified to use runtime `sqlx::query()` and `sqlx::query_as()` instead of the macro versions. See `user_repo.rs` for an example of the runtime approach.

## Features

- **Type-safe queries**: Compile-time SQL verification with sqlx macros
- **Async/await**: Full tokio async support
- **Connection pooling**: Automatic connection management
- **Transaction support**: ACID guarantees for complex operations
- **Error handling**: Comprehensive error mapping to AppError
- **Tracing**: Instrumented methods for observability
- **Testing**: Unit tests for business logic

## LPM Algorithm

The Longest Prefix Match algorithm for rate lookups works as follows:

1. Normalize destination (remove non-digits): `+51-999-888-777` → `51999888777`
2. Generate all prefixes: `["51999888777", "5199988877", ..., "519", "51", "5"]`
3. Query with `WHERE destination_prefix = ANY($1)` for efficient matching
4. Order by `LENGTH(destination_prefix) DESC, priority DESC`
5. Return first match (longest and highest priority)

This provides O(1) lookup time using PostgreSQL's index on `destination_prefix`.

## Dependencies

- `apolo-core`: Domain models and traits
- `sqlx`: PostgreSQL driver with async support
- `tokio`: Async runtime
- `chrono`: Date/time handling
- `uuid`: UUID support for reservations
- `rust_decimal`: Precise decimal arithmetic
- `tracing`: Structured logging

## License

Proprietary - ApoloBilling System
