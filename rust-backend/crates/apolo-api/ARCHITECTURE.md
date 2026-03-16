# CDR API Architecture

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Client Layer                             │
│  (Web Browser, Mobile App, CLI, Other Services)                 │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    HTTP/REST API Layer                           │
│                    (Actix-Web Server)                            │
│                                                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │     CORS     │  │  Auth (JWT)  │  │   Logging    │          │
│  │  Middleware  │  │  Middleware  │  │  Middleware  │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    API Handlers (apolo-api)                      │
│                                                                   │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  CDR Handlers                                             │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌────────────────┐   │  │
│  │  │ list_cdrs   │  │  get_cdr    │  │  export_cdrs   │   │  │
│  │  │             │  │             │  │  (streaming)   │   │  │
│  │  └─────────────┘  └─────────────┘  └────────────────┘   │  │
│  │  ┌─────────────────────────────┐                         │  │
│  │  │   get_cdr_stats             │                         │  │
│  │  │   (with time-series)        │                         │  │
│  │  └─────────────────────────────┘                         │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                   │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  DTOs (Data Transfer Objects)                            │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │  │
│  │  │Request DTOs  │  │Response DTOs │  │ Export DTOs  │   │  │
│  │  │- Filters     │  │- CdrResponse │  │- ExportRow   │   │  │
│  │  │- Pagination  │  │- Stats       │  │- Formats     │   │  │
│  │  └──────────────┘  └──────────────┘  └──────────────┘   │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                Repository Layer (apolo-db)                       │
│                                                                   │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  PgCdrRepository                                          │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      │  │
│  │  │ find_by_id  │  │list_filtered│  │create/update│      │  │
│  │  └─────────────┘  └─────────────┘  └─────────────┘      │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                   │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  SQLx Connection Pool                                     │  │
│  │  - Prepared statements                                    │  │
│  │  - Connection pooling (20 max)                            │  │
│  │  - Async queries                                          │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    PostgreSQL Database                           │
│                                                                   │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  cdrs table                                               │  │
│  │  ┌─────────────────────────────────────────────────────┐ │  │
│  │  │ Columns:                                            │ │  │
│  │  │ - id (bigserial)                                    │ │  │
│  │  │ - call_uuid (text, unique)                          │ │  │
│  │  │ - account_id (integer, indexed)                     │ │  │
│  │  │ - caller_number (text, indexed)                     │ │  │
│  │  │ - called_number (text, indexed)                     │ │  │
│  │  │ - start_time (timestamptz, indexed)                 │ │  │
│  │  │ - duration, billsec (integer)                       │ │  │
│  │  │ - cost, rate_per_minute (numeric)                   │ │  │
│  │  │ - ... (20+ columns)                                 │ │  │
│  │  └─────────────────────────────────────────────────────┘ │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## Request Flow

### 1. List CDRs Flow

```
Client Request
    │
    ▼
GET /api/v1/cdrs?page=1&per_page=50&account_id=123
    │
    ▼
┌─────────────────────────────────────┐
│ 1. Extract Query Parameters         │
│    - Validate with validator crate  │
│    - Parse filters                  │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 2. Call Repository                  │
│    - list_filtered(...)             │
│    - Calculate offset/limit         │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 3. Execute SQL Query                │
│    - Parameterized query            │
│    - Use indexes                    │
│    - LIMIT/OFFSET pagination        │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 4. Apply Additional Filters         │
│    - Direction filter               │
│    - Answered-only filter           │
│    - Hangup cause filter            │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 5. Convert to DTOs                  │
│    - Cdr → CdrResponse              │
│    - Create PaginatedResponse       │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 6. Serialize to JSON                │
│    - Serde serialization            │
│    - HTTP 200 response              │
└─────────────────────────────────────┘
    │
    ▼
JSON Response to Client
```

### 2. Export CDRs Flow (Streaming)

```
Client Request
    │
    ▼
GET /api/v1/cdrs/export?format=csv&limit=100000
    │
    ▼
┌─────────────────────────────────────┐
│ 1. Validate Parameters              │
│    - Format (csv/json/jsonl)        │
│    - Limit (max 1M)                 │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 2. Create Stream                    │
│    - unfold with batch state        │
│    - Initialize offset = 0          │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 3. Stream Loop (per batch)          │
│    ┌───────────────────────────┐    │
│    │ a) Fetch 1000 records     │    │
│    │    from database          │    │
│    └───────────────────────────┘    │
│    ┌───────────────────────────┐    │
│    │ b) Convert to ExportRow   │    │
│    └───────────────────────────┘    │
│    ┌───────────────────────────┐    │
│    │ c) Format (CSV/JSON/JSONL)│    │
│    └───────────────────────────┘    │
│    ┌───────────────────────────┐    │
│    │ d) Send chunk to client   │    │
│    └───────────────────────────┘    │
│    ┌───────────────────────────┐    │
│    │ e) Increment offset       │    │
│    └───────────────────────────┘    │
│    ┌───────────────────────────┐    │
│    │ f) Check if done          │    │
│    └───────────────────────────┘    │
│         │                            │
│         ▼                            │
│    Repeat until all records sent    │
└─────────────────────────────────────┘
    │
    ▼
Streaming Response Complete
(Constant memory: ~200 KB)
```

### 3. Statistics Flow

```
Client Request
    │
    ▼
GET /api/v1/cdrs/stats?group_by=day&start_date=2024-01-01
    │
    ▼
┌─────────────────────────────────────┐
│ 1. Validate Parameters              │
│    - Date range                     │
│    - Group by option                │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 2. Fetch Matching CDRs              │
│    - Large limit (1M)               │
│    - Filtered by criteria           │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 3. Calculate Overall Stats          │
│    - Total/answered/failed calls    │
│    - ASR calculation                │
│    - Sum durations and costs        │
│    - Average calculations           │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 4. Calculate Time Series (optional) │
│    - Group by period                │
│    - Aggregate per period           │
│    - Sort by timestamp              │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ 5. Build CdrStats Response          │
│    - Include overall metrics        │
│    - Include time series            │
└─────────────────────────────────────┘
    │
    ▼
JSON Response to Client
```

## Data Flow Diagram

```
┌────────────┐
│   Client   │
└─────┬──────┘
      │ HTTP Request (JSON)
      ▼
┌────────────────────┐
│  Query/Path/Body   │
│   Extractors       │
└─────┬──────────────┘
      │ CdrFilterParams
      ▼
┌────────────────────┐
│    Validation      │
│   (validator)      │
└─────┬──────────────┘
      │ Validated DTO
      ▼
┌────────────────────┐
│   CDR Handler      │
│  (list_cdrs, etc)  │
└─────┬──────────────┘
      │ Repository Call
      ▼
┌────────────────────┐
│  PgCdrRepository   │
└─────┬──────────────┘
      │ SQL Query
      ▼
┌────────────────────┐
│    PostgreSQL      │
└─────┬──────────────┘
      │ Vec<CdrRow>
      ▼
┌────────────────────┐
│  Row → Cdr Model   │
│   (From trait)     │
└─────┬──────────────┘
      │ Vec<Cdr>
      ▼
┌────────────────────┐
│ Cdr → CdrResponse  │
│   (DTO mapping)    │
└─────┬──────────────┘
      │ Vec<CdrResponse>
      ▼
┌────────────────────┐
│  PaginatedResponse │
│   Construction     │
└─────┬──────────────┘
      │ JSON
      ▼
┌────────────────────┐
│  Serde Serialize   │
└─────┬──────────────┘
      │ HTTP Response
      ▼
┌────────────┐
│   Client   │
└────────────┘
```

## Component Responsibilities

### Handler Layer (`handlers/cdr.rs`)
- **Input validation** - Validate all query parameters
- **Business logic** - Additional filtering, transformations
- **Error handling** - Convert errors to HTTP responses
- **Response formatting** - Create proper DTOs

### Repository Layer (`apolo-db`)
- **Data access** - Execute SQL queries
- **Connection pooling** - Manage database connections
- **Transaction management** - Handle database transactions
- **Model mapping** - Convert database rows to models

### DTO Layer (`dto/`)
- **API contracts** - Define request/response structures
- **Validation rules** - Define constraints
- **Serialization** - JSON encoding/decoding
- **Type safety** - Strong typing for API boundaries

## Performance Characteristics

### Memory Usage

| Operation | Records | Memory |
|-----------|---------|--------|
| List 50 CDRs | 50 | ~10 KB |
| List 1000 CDRs | 1,000 | ~200 KB |
| Export (streaming) | 1,000,000 | ~200 KB (constant) |
| Stats (no grouping) | 100,000 | ~20 MB |
| Stats (grouped) | 100,000 | ~25 MB |

### Database Queries

```sql
-- List CDRs (optimized with indexes)
SELECT ... FROM cdrs
WHERE account_id = $1
  AND start_time >= $2
  AND start_time <= $3
  AND caller_number LIKE $4
ORDER BY start_time DESC
LIMIT $5 OFFSET $6;

-- Count for pagination
SELECT COUNT(*) FROM cdrs
WHERE <same filters>;

-- Export (batched)
SELECT ... FROM cdrs
WHERE <filters>
ORDER BY start_time DESC
LIMIT 1000 OFFSET <batch_offset>;
```

### Indexes Required

```sql
CREATE INDEX idx_cdrs_account_id ON cdrs(account_id);
CREATE INDEX idx_cdrs_start_time ON cdrs(start_time DESC);
CREATE INDEX idx_cdrs_caller ON cdrs(caller_number text_pattern_ops);
CREATE INDEX idx_cdrs_called ON cdrs(called_number text_pattern_ops);
CREATE INDEX idx_cdrs_account_time ON cdrs(account_id, start_time DESC);
```

## Scalability Considerations

### Horizontal Scaling
- **Stateless handlers** - Can run multiple instances
- **Database connection pooling** - Shared resource
- **Load balancing** - Nginx/HAProxy in front

### Vertical Scaling
- **Connection pool size** - Scale with CPU cores
- **Batch size tuning** - Adjust for memory/throughput
- **Index optimization** - Add composite indexes

### Caching Strategy
```
┌──────────────┐
│    Redis     │
│   (Cache)    │
└──────┬───────┘
       │ Cache frequently accessed stats
       │ TTL: 5-10 minutes
       ▼
┌──────────────┐
│   Handler    │
└──────┬───────┘
       │ Cache miss
       ▼
┌──────────────┐
│  PostgreSQL  │
└──────────────┘
```

## Security Architecture

```
┌─────────────────┐
│  Client Request │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  CORS Check     │ ← Only allowed origins
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  JWT Validation │ ← Verify token signature
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Claims Check   │ ← Verify user role/permissions
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Input Valid.   │ ← Prevent injection, validate types
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Handler Logic  │
└─────────────────┘
```

## Error Handling Flow

```
Handler
   │
   ├─ Validation Error → 400 Bad Request
   │
   ├─ Authentication Error → 401 Unauthorized
   │
   ├─ Authorization Error → 403 Forbidden
   │
   ├─ Not Found Error → 404 Not Found
   │
   ├─ Database Error → 500 Internal Server Error
   │
   └─ Success → 200 OK
```

## Monitoring Points

```
┌─────────────────────────────────────┐
│  Metrics to Monitor                 │
├─────────────────────────────────────┤
│  • Request rate (req/s)             │
│  • Response time (p50, p95, p99)    │
│  • Error rate (%)                   │
│  • Database query time              │
│  • Connection pool usage            │
│  • Export throughput (records/s)    │
│  • Memory usage                     │
│  • CPU usage                        │
└─────────────────────────────────────┘
```

This architecture is designed for:
- **High throughput** (10,000+ req/s)
- **Low latency** (< 10ms p50)
- **Memory efficiency** (constant usage for exports)
- **Horizontal scalability** (stateless design)
- **Type safety** (compile-time guarantees)
