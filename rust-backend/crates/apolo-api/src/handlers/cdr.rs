//! CDR API handlers
//!
//! High-performance handlers for CDR listing, retrieval, export, and statistics.
//! Implements efficient streaming for large exports to avoid memory pressure.

use crate::dto::{
    ApiResponse, CdrExportParams, CdrFilterParams, CdrResponse, CdrStats,
    ExportFormat, StatsGroupBy, StatsParams, TimeSeriesPoint,
};
use actix_web::{
    web::{Data, Json, Path, Query},
    HttpResponse, Result,
};
use apolo_auth::AuthenticatedUser;
use apolo_core::{
    error::AppError,
    models::{AuditLogBuilder, Cdr},
    traits::{
        CdrListFilter, CdrRepository, Repository, CDR_LONG_CALL_SECS, CDR_SHORT_CALL_SECS,
    },
    AppResult,
};
use apolo_db::repositories::cdr_repo::{cdr_where, PgCdrRepository};
use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};
use futures::stream;
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::{debug, error, info, instrument, warn};
use validator::Validate;

/// List CDRs with filtering and pagination
///
/// # Errors
///
/// Returns error if database query fails or validation fails.
///
/// # Examples
///
/// ```text
/// GET /api/cdrs?page=1&per_page=50&account_id=123&start_date=2024-01-01
/// ```
#[instrument(skip(db, query))]
pub async fn list_cdrs(
    user: AuthenticatedUser,
    query: Query<CdrFilterParams>,
    db: Data<PgPool>,
) -> Result<HttpResponse> {
    // Validate query parameters
    query.validate().map_err(|e| {
        warn!("Invalid query parameters: {}", e);
        AppError::Validation(e.to_string())
    })?;

    let repo = PgCdrRepository::new(db.get_ref().clone());

    debug!(
        "Listing CDRs: page={}, per_page={}, account_id={:?}, caller={:?}, callee={:?}",
        query.pagination.page,
        query.pagination.per_page,
        query.account_id,
        query.caller,
        query.callee
    );

    // Parse date filters (an unparseable date is a 400, never "no filter")
    let start_date = parse_start_date(&query.start_date)?;
    let end_date = parse_end_date(&query.end_date)?;
    let account_id = resolve_account(db.get_ref(), query.account_id, &query.account).await?;

    // Page 1 takes a snapshot of the newest id; later pages send it back so
    // CDRs written meanwhile don't shift the rows being paged
    let snapshot = match query.max_id {
        Some(id) => id,
        None => repo.max_id().await?,
    };

    let (cdrs, total) = repo
        .list_filtered(
            account_id,
            query.caller.as_deref(),
            query.callee.as_deref(),
            start_date,
            end_date,
            &CdrListFilter {
                direction: query.direction.map(|d| d.as_str().to_string()),
                hangup_cause: query.hangup_cause.clone(),
                answered_only: query.answered_only,
                outcome: query.outcome.clone().filter(|o| !o.is_empty()),
                min_billsec: query.min_billsec,
                max_id: Some(snapshot),
            },
            query.pagination.limit(),
            query.pagination.offset(),
        )
        .await?;

    let mut response_data: Vec<CdrResponse> = cdrs.into_iter().map(CdrResponse::from).collect();
    attach_accounts(db.get_ref(), &mut response_data).await?;
    if masks_numbers(&user) {
        response_data.iter_mut().for_each(mask_response);
    }

    info!(
        "Retrieved {} CDRs out of {} total",
        response_data.len(),
        total
    );

    Ok(HttpResponse::Ok()
        .insert_header(("X-Cdr-Snapshot", snapshot.to_string()))
        .json(query.pagination.paginate(response_data, total)))
}

/// Totals of the filtered set for the CDR page header: ASR, ACD, minutes,
/// calls cut by platform restarts (grouped into outages) and calls flagged
/// for review. Takes the same filters as the list.
///
/// # Errors
///
/// Returns error if validation or the database query fails.
#[instrument(skip(db, query))]
pub async fn cdr_summary(
    _user: AuthenticatedUser,
    query: Query<CdrFilterParams>,
    db: Data<PgPool>,
) -> Result<Json<ApiResponse<serde_json::Value>>> {
    query.validate().map_err(|e| AppError::Validation(e.to_string()))?;

    let start_date = parse_start_date(&query.start_date)?;
    let end_date = parse_end_date(&query.end_date)?;
    let account_id = resolve_account(db.get_ref(), query.account_id, &query.account).await?;
    let where_clause = cdr_where(
        account_id,
        query.caller.as_deref(),
        query.callee.as_deref(),
        start_date,
        end_date,
        &CdrListFilter {
            direction: query.direction.map(|d| d.as_str().to_string()),
            hangup_cause: query.hangup_cause.clone(),
            answered_only: query.answered_only,
            outcome: query.outcome.clone().filter(|o| !o.is_empty()),
            min_billsec: query.min_billsec,
            max_id: query.max_id,
        },
    );
    let summary = PgCdrRepository::new(db.get_ref().clone())
        .summary(&where_clause)
        .await?;

    Ok(Json(ApiResponse::success(serde_json::json!({
        "total": summary.total,
        "answered": summary.answered,
        "billsec": summary.billsec,
        "without_customer": summary.without_customer,
        "long_calls": summary.long_calls,
        "short_answered": summary.short_answered,
        "long_call_secs": CDR_LONG_CALL_SECS,
        "short_call_secs": CDR_SHORT_CALL_SECS,
        "system_cut": summary.system_cut,
        "system_cut_answered": summary.system_cut_answered,
        "outages": group_outages(&summary.system_cut_minutes),
    }))))
}

/// Calls cut by a restart end within a few minutes of each other: minutes
/// less than `OUTAGE_GAP_MINUTES` apart are one outage
fn group_outages(minutes: &[(DateTime<Utc>, i64, i64)]) -> Vec<serde_json::Value> {
    const OUTAGE_GAP_MINUTES: i64 = 5;
    let mut groups: Vec<(DateTime<Utc>, DateTime<Utc>, i64, i64)> = Vec::new();
    for &(minute, calls, answered) in minutes {
        match groups.last_mut() {
            Some(g) if minute - g.1 <= Duration::minutes(OUTAGE_GAP_MINUTES) => {
                g.1 = minute;
                g.2 += calls;
                g.3 += answered;
            }
            _ => groups.push((minute, minute, calls, answered)),
        }
    }
    groups
        .into_iter()
        .map(|(from, to, calls, answered)| {
            serde_json::json!({
                "from": from,
                "to": to + Duration::minutes(1),
                "calls": calls,
                "answered": answered,
            })
        })
        .collect()
}

/// `account` (customer number) → account_id; an unknown number matches nothing
async fn resolve_account(
    pool: &PgPool,
    account_id: Option<i32>,
    account: &Option<String>,
) -> AppResult<Option<i32>> {
    let Some(number) = account.as_deref().map(str::trim).filter(|n| !n.is_empty()) else {
        return Ok(account_id);
    };
    let row: Option<(i32,)> = sqlx::query_as("SELECT id FROM accounts WHERE account_number = $1")
        .bind(number)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to resolve customer: {}", e)))?;
    Ok(Some(row.map_or(-1, |r| r.0)))
}

/// Fill customer number/name for the CDRs that have one
async fn attach_accounts(pool: &PgPool, rows: &mut [CdrResponse]) -> AppResult<()> {
    let mut ids: Vec<i32> = rows.iter().filter_map(|r| r.account_id).collect();
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        return Ok(());
    }
    let accounts: Vec<(i32, String, Option<String>)> = sqlx::query_as(
        "SELECT id, account_number, account_name FROM accounts WHERE id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to load customers: {}", e)))?;
    let by_id: std::collections::HashMap<i32, (String, Option<String>)> =
        accounts.into_iter().map(|(id, n, name)| (id, (n, name))).collect();
    for row in rows.iter_mut() {
        if let Some((number, name)) = row.account_id.and_then(|id| by_id.get(&id)) {
            row.account_number = Some(number.clone());
            row.account_name = name.clone();
        }
    }
    Ok(())
}

/// Phone numbers are personal data (Ley 29733): operators see them masked,
/// admins in full. Filters still match the full number server-side.
fn masks_numbers(user: &AuthenticatedUser) -> bool {
    !user.is_admin()
}

/// `993171615` → `9931***615`; short numbers keep only their first digits
fn mask_number(n: &str) -> String {
    let chars: Vec<char> = n.chars().collect();
    match chars.len() {
        0..=3 => n.to_string(),
        4..=6 => format!("{}***", chars[..2].iter().collect::<String>()),
        len => format!(
            "{}***{}",
            chars[..4].iter().collect::<String>(),
            chars[len - 3..].iter().collect::<String>()
        ),
    }
}

fn mask_response(r: &mut CdrResponse) {
    r.caller = mask_number(&r.caller);
    r.callee = mask_number(&r.callee);
    r.caller_e164 = r.caller_e164.as_deref().map(mask_number);
    r.destination = r.destination.as_deref().map(mask_number);
}

/// Get a single CDR by ID
///
/// # Errors
///
/// Returns 404 if CDR not found, or error if database query fails.
///
/// # Examples
///
/// ```text
/// GET /api/cdrs/12345
/// ```
#[instrument(skip(db))]
pub async fn get_cdr(
    user: AuthenticatedUser,
    path: Path<i64>,
    db: Data<PgPool>,
) -> Result<Json<ApiResponse<CdrResponse>>> {
    let cdr_id = path.into_inner();
    debug!("Fetching CDR with id: {}", cdr_id);

    let repo = PgCdrRepository::new(db.get_ref().clone());

    let cdr = repo
        .find_by_id(cdr_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("CDR with id {} not found", cdr_id)))?;

    info!("Retrieved CDR: {}", cdr_id);

    let mut response = vec![CdrResponse::from(cdr)];
    attach_accounts(db.get_ref(), &mut response).await?;
    let mut response = response.remove(0);
    if masks_numbers(&user) {
        mask_response(&mut response);
    }
    Ok(Json(ApiResponse::success(response)))
}

/// Export CDRs in various formats (CSV, JSON, JSONL)
///
/// Implements efficient streaming to handle millions of records without
/// loading everything into memory. Uses a streaming response with chunks.
///
/// # Errors
///
/// Returns error if database query fails or streaming fails.
///
/// # Examples
///
/// ```text
/// GET /api/cdrs/export?format=csv&account_id=123&start_date=2024-01-01&limit=100000
/// ```
#[allow(clippy::too_many_lines)]
#[instrument(skip(db, query))]
pub async fn export_cdrs(
    user: AuthenticatedUser,
    query: Query<CdrExportParams>,
    db: Data<PgPool>,
) -> Result<HttpResponse> {
    use chrono_tz::America::Lima;

    // Validate query parameters
    query.validate().map_err(|e| {
        warn!("Invalid export parameters: {}", e);
        AppError::Validation(e.to_string())
    })?;

    // An export always has a bounded window: start_date is required, the end
    // defaults to now (frozen here so later batches don't pick up new CDRs)
    // and the window is capped, so a bad parameter can't dump the whole table.
    let start = parse_start_date(&query.start_date)?.ok_or_else(|| {
        AppError::Validation("start_date es obligatorio para exportar".to_string())
    })?;
    let end = parse_end_date(&query.end_date)?.unwrap_or_else(Utc::now);
    if end < start {
        return Err(AppError::Validation("end_date es anterior a start_date".to_string()).into());
    }
    if end - start > Duration::days(MAX_EXPORT_DAYS) + Duration::hours(1) {
        return Err(AppError::Validation(format!(
            "El export admite como máximo {} días por archivo",
            MAX_EXPORT_DAYS
        ))
        .into());
    }

    let params = query.into_inner();
    let account_id = resolve_account(db.get_ref(), params.account_id, &params.account).await?;
    let masked = masks_numbers(&user);

    info!(
        "Starting CDR export: user={}, format={:?}, limit={}, start={}, end={}, direction={:?}",
        user.username, params.format, params.limit, start, end, params.direction
    );

    // Who exported what: the file itself stays plain CSV
    if let Ok(audit) = AuditLogBuilder::default()
        .username(user.username.clone())
        .action("export")
        .entity_type("cdrs")
        .details(serde_json::json!({
            "format": params.format.extension(),
            "start_date": start.to_rfc3339(),
            "end_date": end.to_rfc3339(),
            "direction": params.direction.map(|d| d.as_str()),
            "caller": params.caller,
            "callee": params.callee,
            "account_id": account_id,
            "account": params.account,
            "outcome": params.outcome,
            "min_billsec": params.min_billsec,
            "masked": masked,
            "limit": params.limit,
        }))
        .build()
    {
        audit.insert(db.get_ref()).await;
    }

    let format = params.format;
    let pool = db.get_ref().clone();

    // Filename = the exported window in Lima time (+ direction)
    let lima = |dt: DateTime<Utc>| dt.with_timezone(&Lima).format("%Y%m%d-%H%M").to_string();
    let filename = format!(
        "cdrs_{}_{}{}.{}",
        lima(start),
        lima(end),
        params
            .direction
            .map(|d| format!("_{}", d.as_str()))
            .unwrap_or_default(),
        format.extension()
    );

    // Start streaming response
    let mut response = HttpResponse::Ok();
    response.content_type(format.content_type()).insert_header((
        "Content-Disposition",
        format!("attachment; filename=\"{}\"", filename),
    ));

    let cursor = ExportCursor::new(pool, params, account_id, start, end);
    Ok(response.streaming(export_stream(cursor, masked)))
}

/// Get CDR statistics
///
/// Calculates aggregated statistics including call counts, ASR and durations.
/// Optionally groups results by time periods.
///
/// # Errors
///
/// Returns error if database query fails or validation fails.
///
/// # Examples
///
/// ```text
/// GET /api/cdrs/stats?account_id=123&start_date=2024-01-01&group_by=day
/// ```
#[instrument(skip(db, query))]
pub async fn get_cdr_stats(
    _user: AuthenticatedUser,
    query: Query<StatsParams>,
    db: Data<PgPool>,
) -> Result<Json<ApiResponse<CdrStats>>> {
    // Validate query parameters
    query.validate().map_err(|e| {
        warn!("Invalid stats parameters: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(
        "Calculating CDR stats: account_id={:?}, start={:?}, end={:?}, group_by={:?}",
        query.account_id, query.start_date, query.end_date, query.group_by
    );

    let repo = PgCdrRepository::new(db.get_ref().clone());

    // Fetch all matching CDRs (no pagination for stats)
    let (cdrs, _) = repo
        .list_filtered(
            query.account_id,
            None,
            None,
            query.start_date,
            query.end_date,
            &CdrListFilter::default(),
            1_000_000, // Large limit for stats
            0,
        )
        .await?;

    // Calculate overall statistics
    let stats = calculate_stats(&cdrs);

    // Calculate time series if grouping requested
    let time_series = if let Some(group_by) = query.group_by {
        Some(calculate_time_series(&cdrs, group_by))
    } else {
        None
    };

    let result = CdrStats {
        time_series,
        ..stats
    };

    info!(
        "Calculated stats: total_calls={}, asr={}%",
        result.total_calls, result.asr
    );

    Ok(Json(ApiResponse::success(result)))
}

// ============================================================================
// Helper Functions - Streaming
// ============================================================================

/// Walks an export window newest-first in 1-hour slices. Each slice is one
/// indexed query without OFFSET (a month is ~744 queries of at most a few
/// thousand rows instead of OFFSET scans of up to ~850k), and consecutive
/// slices are [lo, hi] / [.., lo - 1µs], so no CDR is skipped or repeated.
struct ExportCursor {
    pool: PgPool,
    repo: PgCdrRepository,
    params: CdrExportParams,
    account_id: Option<i32>,
    start: DateTime<Utc>,
    next_end: Option<DateTime<Utc>>,
    remaining: i64,
}

impl ExportCursor {
    fn new(
        pool: PgPool,
        params: CdrExportParams,
        account_id: Option<i32>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Self {
        let remaining = params.limit;
        Self {
            repo: PgCdrRepository::new(pool.clone()),
            pool,
            params,
            account_id,
            start,
            next_end: Some(end),
            remaining,
        }
    }

    /// Next non-empty slice, or None once the window or the row limit is exhausted
    async fn next_batch(&mut self) -> AppResult<Option<Vec<CdrResponse>>> {
        while let Some(hi) = self.next_end {
            if self.remaining <= 0 {
                break;
            }
            let lo = (hi - Duration::hours(1)).max(self.start);
            self.next_end = (lo > self.start).then(|| lo - Duration::microseconds(1));

            let (cdrs, _) = self
                .repo
                .list_filtered(
                    self.account_id,
                    self.params.caller.as_deref(),
                    self.params.callee.as_deref(),
                    Some(lo),
                    Some(hi),
                    &CdrListFilter {
                        direction: self.params.direction.map(|d| d.as_str().to_string()),
                        outcome: self.params.outcome.clone().filter(|o| !o.is_empty()),
                        min_billsec: self.params.min_billsec,
                        ..Default::default()
                    },
                    self.remaining,
                    0,
                )
                .await?;
            if !cdrs.is_empty() {
                self.remaining -= cdrs.len() as i64;
                let mut rows: Vec<CdrResponse> = cdrs.into_iter().map(CdrResponse::from).collect();
                attach_accounts(&self.pool, &mut rows).await?;
                return Ok(Some(rows));
            }
        }
        Ok(None)
    }
}

/// One CSV line (RFC 4180, formula-guarded, Lima times), in `CSV_HEADER` order
fn csv_line(cdr: &CdrResponse) -> String {
    let lima_time = |dt: Option<DateTime<Utc>>| {
        dt.map(|t| {
            t.with_timezone(&chrono_tz::America::Lima)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_default()
    };
    let opt = |v: &Option<String>| v.clone().unwrap_or_default();
    let fields = [
        cdr.id.to_string(),
        cdr.call_uuid.clone(),
        cdr.account_id.map(|id| id.to_string()).unwrap_or_default(),
        opt(&cdr.account_number),
        cdr.caller.clone(),
        opt(&cdr.caller_e164),
        cdr.callee.clone(),
        opt(&cdr.destination),
        lima_time(Some(cdr.start_time)),
        lima_time(cdr.answer_time),
        lima_time(Some(cdr.end_time)),
        cdr.duration.to_string(),
        cdr.billsec.to_string(),
        cdr.hangup_cause.clone(),
        cdr.sip_code.map(|c| c.to_string()).unwrap_or_default(),
        cdr.direction.clone(),
        opt(&cdr.source_ip),
    ];
    let mut line = fields.iter().map(|f| csv_field(f)).collect::<Vec<_>>().join(",");
    line.push('\n');
    line
}

/// Streaming export body. A database error aborts the stream (the download
/// fails visibly) instead of ending it early as if the file were complete.
fn export_stream(
    cursor: ExportCursor,
    masked: bool,
) -> impl futures::Stream<Item = Result<actix_web::web::Bytes, actix_web::Error>> {
    #[derive(Clone, Copy)]
    enum Phase {
        Open,
        Body,
        Closed,
    }

    let format = cursor.params.format;

    stream::unfold(
        (cursor, Phase::Open, true),
        move |(mut cursor, phase, first)| async move {
            let (chunk, next) = match phase {
                Phase::Closed => return None,
                Phase::Open => {
                    let head = match format {
                        ExportFormat::Csv => format!("{}\n", CSV_HEADER),
                        ExportFormat::Json => "[".to_string(),
                        ExportFormat::Jsonl => String::new(),
                    };
                    (Ok(head), Phase::Body)
                }
                Phase::Body => match cursor.next_batch().await {
                    Ok(Some(cdrs)) => {
                        let mut out = String::new();
                        let mut first_row = first;
                        for mut cdr in cdrs {
                            if masked {
                                mask_response(&mut cdr);
                            }
                            match format {
                                ExportFormat::Csv => out.push_str(&csv_line(&cdr)),
                                ExportFormat::Json | ExportFormat::Jsonl => {
                                    let Ok(json) = serde_json::to_string(&cdr) else {
                                        continue;
                                    };
                                    if matches!(format, ExportFormat::Json) && !first_row {
                                        out.push(',');
                                    }
                                    out.push_str(&json);
                                    if matches!(format, ExportFormat::Jsonl) {
                                        out.push('\n');
                                    }
                                }
                            }
                            first_row = false;
                        }
                        return Some((
                            Ok(actix_web::web::Bytes::from(out)),
                            (cursor, Phase::Body, first_row),
                        ));
                    }
                    Ok(None) => {
                        let tail = if matches!(format, ExportFormat::Json) {
                            "]"
                        } else {
                            ""
                        };
                        (Ok(tail.to_string()), Phase::Closed)
                    }
                    Err(e) => {
                        error!("Error fetching CDRs for export: {}", e);
                        (
                            Err(actix_web::error::ErrorInternalServerError(
                                "CDR export aborted",
                            )),
                            Phase::Closed,
                        )
                    }
                },
            };
            Some((
                chunk.map(actix_web::web::Bytes::from),
                (cursor, next, first),
            ))
        },
    )
}

// ============================================================================
// Helper Functions - Statistics
// ============================================================================

/// Calculate overall statistics from a list of CDRs
fn calculate_stats(cdrs: &[Cdr]) -> CdrStats {
    let total_calls = cdrs.len() as i64;

    if total_calls == 0 {
        return CdrStats {
            total_calls: 0,
            answered_calls: 0,
            failed_calls: 0,
            asr: Decimal::ZERO,
            total_duration: 0,
            total_billsec: 0,
            avg_duration: Decimal::ZERO,
            avg_billsec: Decimal::ZERO,
            time_series: None,
        };
    }

    let answered_calls = cdrs.iter().filter(|cdr| cdr.was_answered()).count() as i64;
    let failed_calls = total_calls - answered_calls;

    let asr = if total_calls > 0 {
        Decimal::from(answered_calls) / Decimal::from(total_calls) * Decimal::from(100)
    } else {
        Decimal::ZERO
    };

    let total_duration: i64 = cdrs.iter().map(|cdr| i64::from(cdr.duration)).sum();
    let total_billsec: i64 = cdrs.iter().map(|cdr| i64::from(cdr.billsec)).sum();

    let avg_duration = Decimal::from(total_duration) / Decimal::from(total_calls);
    let avg_billsec = Decimal::from(total_billsec) / Decimal::from(total_calls);

    CdrStats {
        total_calls,
        answered_calls,
        failed_calls,
        asr,
        total_duration,
        total_billsec,
        avg_duration,
        avg_billsec,
        time_series: None,
    }
}

/// Calculate time series statistics grouped by period
fn calculate_time_series(cdrs: &[Cdr], group_by: StatsGroupBy) -> Vec<TimeSeriesPoint> {
    use std::collections::HashMap;

    let mut groups: HashMap<String, (DateTime<Utc>, Vec<&Cdr>)> = HashMap::new();

    // Group CDRs by period
    for cdr in cdrs {
        let (period_key, period_start) = get_period_key(&cdr.start_time, group_by);

        groups
            .entry(period_key)
            .or_insert_with(|| (period_start, Vec::new()))
            .1
            .push(cdr);
    }

    // Calculate stats for each period
    let mut time_series: Vec<TimeSeriesPoint> = groups
        .into_iter()
        .map(|(period, (timestamp, cdrs))| {
            let calls = cdrs.len() as i64;
            let duration: i64 = cdrs.iter().map(|cdr| i64::from(cdr.duration)).sum();

            TimeSeriesPoint {
                period,
                timestamp,
                calls,
                duration,
            }
        })
        .collect();

    // Sort by timestamp
    time_series.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    time_series
}

/// Get period key and start timestamp for grouping
fn get_period_key(dt: &DateTime<Utc>, group_by: StatsGroupBy) -> (String, DateTime<Utc>) {
    match group_by {
        StatsGroupBy::Hour => {
            let start = Utc
                .with_ymd_and_hms(dt.year(), dt.month(), dt.day(), dt.hour(), 0, 0)
                .unwrap();
            let key = start.format("%Y-%m-%d %H:00").to_string();
            (key, start)
        }
        StatsGroupBy::Day => {
            let start = Utc
                .with_ymd_and_hms(dt.year(), dt.month(), dt.day(), 0, 0, 0)
                .unwrap();
            let key = start.format("%Y-%m-%d").to_string();
            (key, start)
        }
        StatsGroupBy::Week => {
            // Start of week (Monday)
            let weekday = dt.weekday().num_days_from_monday();
            let start = *dt - Duration::days(i64::from(weekday));
            let start = Utc
                .with_ymd_and_hms(start.year(), start.month(), start.day(), 0, 0, 0)
                .unwrap();
            let key = format!("Week of {}", start.format("%Y-%m-%d"));
            (key, start)
        }
        StatsGroupBy::Month => {
            let start = Utc
                .with_ymd_and_hms(dt.year(), dt.month(), 1, 0, 0, 0)
                .unwrap();
            let key = start.format("%Y-%m").to_string();
            (key, start)
        }
    }
}

/// Longest window a single export may cover
const MAX_EXPORT_DAYS: i64 = 31;

/// CSV header; times are Lima local time (UTC-5, no DST)
const CSV_HEADER: &str = "id,call_uuid,account_id,customer,caller,caller (E.164),callee,destination (E.164),start_time (GMT-5),answer_time (GMT-5),end_time (GMT-5),duration,billsec,hangup_cause,sip_code,direction,source_ip";

/// RFC 4180 field: quoted when it holds a comma, quote or line break; values a
/// spreadsheet would run as a formula (= + - @, tab, CR) are prefixed with '.
/// A bare E.164 number (`+` and 1-15 digits) can't carry a formula and is left
/// intact, so the destination column stays machine-readable.
fn csv_field(value: &str) -> String {
    let e164 = value
        .strip_prefix('+')
        .is_some_and(|d| (1..=15).contains(&d.len()) && d.bytes().all(|b| b.is_ascii_digit()));
    let guarded = if !e164 && value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{}", value)
    } else {
        value.to_string()
    };
    if guarded.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", guarded.replace('"', "\"\""))
    } else {
        guarded
    }
}

/// Parse a date filter: RFC 3339 with an explicit offset, or a bare
/// `YYYY-MM-DD` read as a Lima calendar day (start or end of it). Anything else,
/// including a timestamp without offset, is a 400 rather than "no filter".
fn parse_date_bound(
    name: &str,
    date_str: &Option<String>,
    end_of_day: bool,
) -> std::result::Result<Option<DateTime<Utc>>, AppError> {
    use chrono_tz::America::Lima;

    let s = match date_str.as_deref().map(str::trim) {
        None | Some("") => return Ok(None),
        Some(s) => s,
    };

    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(Some(dt.with_timezone(&Utc)));
    }

    if let Ok(naive) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let naive_dt = if end_of_day {
            naive.and_hms_micro_opt(23, 59, 59, 999_999)
        } else {
            naive.and_hms_opt(0, 0, 0)
        };
        // Lima has no DST, so local times are never ambiguous
        if let Some(lima_dt) = naive_dt.and_then(|n| Lima.from_local_datetime(&n).single()) {
            return Ok(Some(lima_dt.with_timezone(&Utc)));
        }
    }

    Err(AppError::Validation(format!(
        "{} inválida: '{}'. Use RFC 3339 con zona (2026-10-24T00:00:00-05:00) o YYYY-MM-DD (día en hora de Lima)",
        name, s
    )))
}

/// Parse start date (beginning of the day in Lima for a bare date)
fn parse_start_date(
    date_str: &Option<String>,
) -> std::result::Result<Option<DateTime<Utc>>, AppError> {
    parse_date_bound("start_date", date_str, false)
}

/// Parse end date (end of the day in Lima for a bare date)
fn parse_end_date(
    date_str: &Option<String>,
) -> std::result::Result<Option<DateTime<Utc>>, AppError> {
    parse_date_bound("end_date", date_str, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use apolo_core::models::Cdr;

    #[test]
    fn test_mask_number() {
        assert_eq!(mask_number("993171615"), "9931***615");
        assert_eq!(mask_number("+51993171615"), "+519***615");
        assert_eq!(mask_number("1001"), "10***");
        assert_eq!(mask_number("123"), "123");
    }

    #[test]
    fn test_group_outages() {
        let t = |h, m| Utc.with_ymd_and_hms(2026, 10, 25, h, m, 0).unwrap();
        let minutes = [
            (t(5, 36), 10, 8),
            (t(5, 37), 2, 1),
            (t(14, 59), 30, 25),
            (t(15, 2), 5, 4),
            (t(15, 9), 40, 30),
        ];
        let outages = group_outages(&minutes);
        assert_eq!(outages.len(), 3);
        assert_eq!(outages[0]["calls"], 12);
        assert_eq!(outages[1]["calls"], 35);
        assert_eq!(outages[2]["answered"], 30);
    }

    #[test]
    fn test_csv_field_quotes_and_formula_guard() {
        assert_eq!(csv_field("912345678"), "912345678");
        assert_eq!(csv_field("+18505551234"), "+18505551234");
        assert_eq!(csv_field("+1+1"), "'+1+1");
        assert_eq!(csv_field("+cmd|' /C calc'!A0"), "'+cmd|' /C calc'!A0");
        assert_eq!(csv_field("-1"), "'-1");
        assert_eq!(csv_field("=HYPERLINK(1)"), "'=HYPERLINK(1)");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field(""), "");
    }

    #[test]
    fn test_date_bounds_strict() {
        let some = |s: &str| Some(s.to_string());
        // Bare date = Lima calendar day
        assert_eq!(
            parse_start_date(&some("2026-10-24"))
                .unwrap()
                .unwrap()
                .to_rfc3339(),
            "2026-10-24T05:00:00+00:00"
        );
        assert_eq!(
            parse_end_date(&some("2026-10-24"))
                .unwrap()
                .unwrap()
                .to_rfc3339(),
            "2026-10-25T04:59:59.999999+00:00"
        );
        // RFC 3339 with offset
        assert_eq!(
            parse_start_date(&some("2026-10-24T00:00:00.000-05:00"))
                .unwrap()
                .unwrap()
                .to_rfc3339(),
            "2026-10-24T05:00:00+00:00"
        );
        // Empty / missing = no bound
        assert!(parse_start_date(&None).unwrap().is_none());
        assert!(parse_start_date(&some("")).unwrap().is_none());
        // Garbage or a timestamp without offset is rejected, never ignored
        assert!(parse_start_date(&some("hola")).is_err());
        assert!(parse_end_date(&some("2026-10-24T00:00:00")).is_err());
        assert!(parse_end_date(&some("2026-13-40")).is_err());
    }

    #[test]
    fn test_calculate_stats_empty() {
        let cdrs: Vec<Cdr> = vec![];
        let stats = calculate_stats(&cdrs);

        assert_eq!(stats.total_calls, 0);
        assert_eq!(stats.answered_calls, 0);
        assert_eq!(stats.asr, Decimal::ZERO);
    }

    #[test]
    fn test_calculate_stats() {
        let mut cdr1 = Cdr::default();
        cdr1.duration = 60;
        cdr1.billsec = 55;
        cdr1.answer_time = Some(Utc::now());

        let mut cdr2 = Cdr::default();
        cdr2.duration = 30;
        cdr2.billsec = 0;
        cdr2.answer_time = None;

        let cdrs = vec![cdr1, cdr2];
        let stats = calculate_stats(&cdrs);

        assert_eq!(stats.total_calls, 2);
        assert_eq!(stats.answered_calls, 1);
        assert_eq!(stats.failed_calls, 1);
        assert_eq!(stats.total_duration, 90);
    }

    #[test]
    fn test_get_period_key() {
        let dt = Utc.with_ymd_and_hms(2024, 1, 15, 14, 30, 0).unwrap();

        let (key, _) = get_period_key(&dt, StatsGroupBy::Hour);
        assert_eq!(key, "2024-01-15 14:00");

        let (key, _) = get_period_key(&dt, StatsGroupBy::Day);
        assert_eq!(key, "2024-01-15");

        let (key, _) = get_period_key(&dt, StatsGroupBy::Month);
        assert_eq!(key, "2024-01");
    }
}
