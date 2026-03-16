# Quick Start Guide - CDR API Handlers

## Installation

Add to your `Cargo.toml`:
```toml
[dependencies]
apolo-api = { path = "../crates/apolo-api" }
```

## Basic Setup (5 minutes)

### 1. Create your main application

```rust
use actix_web::{web, App, HttpServer};
use apolo_api::handlers::cdr;
use sqlx::postgres::PgPoolOptions;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Database connection
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect("postgresql://user:pass@localhost/apolo_billing")
        .await
        .expect("Failed to connect to database");

    // Start server
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .service(
                web::scope("/api/v1/cdrs")
                    .route("", web::get().to(cdr::list_cdrs))
                    .route("/{id}", web::get().to(cdr::get_cdr))
                    .route("/export", web::get().to(cdr::export_cdrs))
                    .route("/stats", web::get().to(cdr::get_cdr_stats))
            )
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
```

### 2. Run the server

```bash
cargo run --release
```

### 3. Test the API

```bash
# List CDRs
curl http://localhost:8080/api/v1/cdrs?page=1&per_page=10

# Get single CDR
curl http://localhost:8080/api/v1/cdrs/1

# Export to CSV
curl http://localhost:8080/api/v1/cdrs/export?format=csv -o export.csv

# Get statistics
curl http://localhost:8080/api/v1/cdrs/stats?group_by=day
```

## Common Use Cases

### List CDRs with Filters

```rust
// In your handler or client code:
GET /api/v1/cdrs?account_id=123&start_date=2024-01-01&end_date=2024-01-31&per_page=100
```

**Response:**
```json
{
  "data": [...],
  "pagination": {
    "total": 1500,
    "page": 1,
    "per_page": 100,
    "total_pages": 15
  }
}
```

### Export Large Dataset

```rust
// Stream 1 million records to CSV
GET /api/v1/cdrs/export?format=csv&start_date=2024-01-01&limit=1000000
```

**Features:**
- Constant memory usage (~200 KB)
- Automatic chunking
- Progress streaming

### Calculate Statistics

```rust
// Get daily stats for January 2024
GET /api/v1/cdrs/stats?start_date=2024-01-01&end_date=2024-01-31&group_by=day
```

**Response:**
```json
{
  "data": {
    "total_calls": 10000,
    "answered_calls": 9500,
    "failed_calls": 500,
    "asr": "95.00",
    "total_cost": "5000.00",
    "time_series": [
      {
        "period": "2024-01-01",
        "calls": 323,
        "cost": "161.50",
        "duration": 19380
      }
    ]
  }
}
```

## Adding Authentication

```rust
use apolo_auth::middleware::AuthenticatedUser;

async fn protected_list_cdrs(
    user: AuthenticatedUser,  // Requires JWT token
    query: Query<CdrFilterParams>,
    db: Data<PgPool>,
) -> Result<Json<PaginatedResponse<CdrResponse>>> {
    // Only show CDRs for user's account
    let mut filter = query.into_inner();
    filter.account_id = Some(user.claims.account_id);

    // ... rest of handler
}
```

## Performance Tuning

### Database Indexes (Required!)

```sql
-- Essential indexes for performance
CREATE INDEX idx_cdrs_account_id ON cdrs(account_id);
CREATE INDEX idx_cdrs_start_time ON cdrs(start_time DESC);
CREATE INDEX idx_cdrs_caller ON cdrs(caller_number text_pattern_ops);
CREATE INDEX idx_cdrs_called ON cdrs(called_number text_pattern_ops);

-- Composite index for common queries
CREATE INDEX idx_cdrs_account_time ON cdrs(account_id, start_time DESC);
```

### Connection Pool Tuning

```rust
let pool = PgPoolOptions::new()
    .max_connections(20)        // Adjust based on CPU cores
    .min_connections(5)         // Keep warm connections
    .acquire_timeout(Duration::from_secs(3))
    .connect(&database_url)
    .await?;
```

### Pagination Best Practices

```rust
// Good: Reasonable page size
?per_page=50

// Avoid: Too large (slow queries, high memory)
?per_page=10000

// Good: Use exports for large datasets
?format=csv&limit=100000
```

## Error Handling

All handlers return proper HTTP status codes:

```rust
// 200 OK - Success
{"data": {...}}

// 400 Bad Request - Validation error
{"error": "validation_error", "message": "page must be >= 1", "status": 400}

// 404 Not Found - Resource not found
{"error": "not_found", "message": "CDR with id 12345 not found", "status": 404}

// 500 Internal Server Error - Server error
{"error": "internal_error", "message": "Database connection failed", "status": 500}
```

## Testing

### Unit Tests

```bash
cargo test --package apolo-api
```

### Benchmarks

```bash
cargo bench --package apolo-api
```

### Load Testing

```bash
# Install Apache Bench
apt-get install apache2-utils

# Test list endpoint
ab -n 10000 -c 100 "http://localhost:8080/api/v1/cdrs?page=1&per_page=50"

# Test export endpoint
ab -n 100 -c 10 "http://localhost:8080/api/v1/cdrs/export?format=csv&limit=10000"
```

## Monitoring

### Add Prometheus Metrics

```rust
use actix_web_prom::PrometheusMetricsBuilder;

let prometheus = PrometheusMetricsBuilder::new("api")
    .endpoint("/metrics")
    .build()
    .unwrap();

App::new()
    .wrap(prometheus.clone())
    .service(...)
```

### Structured Logging

```rust
use tracing_subscriber;

tracing_subscriber::fmt()
    .with_target(false)
    .with_thread_ids(true)
    .with_level(true)
    .json()
    .init();
```

## Common Pitfalls

### ❌ Don't: Load all records for statistics
```rust
// BAD - Loads millions of records into memory
let all_cdrs = repo.find_all(999999999, 0).await?;
```

### ✅ Do: Use database aggregations
```rust
// GOOD - Aggregate in database, return summary
let stats = repo.calculate_stats_in_db(filters).await?;
```

### ❌ Don't: Use exports for small datasets
```rust
// BAD - Overhead of streaming for 10 records
export_cdrs(query).await?
```

### ✅ Do: Use list endpoint for small datasets
```rust
// GOOD - Fast, simple response
list_cdrs(query).await?
```

### ❌ Don't: Use JSON format for very large exports
```rust
// BAD - Holds entire array in memory before sending
?format=json&limit=1000000
```

### ✅ Do: Use JSONL for large exports
```rust
// GOOD - Streams one record at a time
?format=jsonl&limit=1000000
```

## Advanced Usage

### Custom Filters

```rust
// Combine multiple filters
GET /api/v1/cdrs?
    account_id=123&
    caller=5199&           // Prefix match
    direction=outbound&
    answered_only=true&
    start_date=2024-01-01T00:00:00Z&
    end_date=2024-01-31T23:59:59Z&
    page=1&
    per_page=100
```

### Statistics Grouping

```rust
// Hourly breakdown
?group_by=hour&start_date=2024-01-20&end_date=2024-01-21

// Daily breakdown
?group_by=day&start_date=2024-01-01&end_date=2024-01-31

// Weekly breakdown
?group_by=week&start_date=2024-01-01&end_date=2024-03-31

// Monthly breakdown
?group_by=month&start_date=2024-01-01&end_date=2024-12-31
```

### Export with Filters

```rust
// Export only failed calls
?format=csv&hangup_cause=NO_ANSWER&limit=50000

// Export specific account's calls
?format=jsonl&account_id=123&start_date=2024-01-01
```

## Production Checklist

- [ ] Database indexes created
- [ ] Connection pool configured
- [ ] Authentication middleware added
- [ ] Rate limiting enabled
- [ ] Monitoring/metrics configured
- [ ] Error logging set up
- [ ] CORS configured for web clients
- [ ] HTTPS/TLS enabled
- [ ] Backup strategy for database
- [ ] Load testing completed

## Support

- **Documentation:** See [README.md](README.md)
- **Examples:** See [examples/](examples/)
- **Tests:** See [tests/](tests/)
- **Benchmarks:** See [benches/](benches/)

## Next Steps

1. Review the [full documentation](README.md)
2. Check out the [example application](examples/cdr_api_usage.rs)
3. Run the [benchmarks](benches/cdr_benchmarks.rs)
4. Integrate into your system
5. Add custom business logic as needed
