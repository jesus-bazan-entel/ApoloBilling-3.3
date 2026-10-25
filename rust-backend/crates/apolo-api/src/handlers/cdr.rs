//! CDR API handlers
//!
//! High-performance handlers for CDR listing, retrieval, export, and statistics.
//! Implements efficient streaming for large exports to avoid memory pressure.

use crate::dto::{
    ApiResponse, CdrExportParams, CdrExportRow, CdrFilterParams, CdrResponse, CdrStats,
    ExportFormat, StatsGroupBy, StatsParams, TimeSeriesPoint,
};
use actix_web::{
    web::{Data, Json, Path, Query},
    HttpResponse, Result,
};
use apolo_core::{
    error::AppError,
    models::{AuditLogBuilder, Cdr},
    traits::{CdrListFilter, CdrRepository, PaginatedResponse, Repository},
    AppResult,
};
use apolo_auth::AuthenticatedUser;
use apolo_db::repositories::cdr_repo::PgCdrRepository;
use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};
use futures::{stream, StreamExt};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::io::Write;
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
    _user: AuthenticatedUser,
    query: Query<CdrFilterParams>,
    db: Data<PgPool>,
) -> Result<Json<PaginatedResponse<CdrResponse>>> {
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

    // Query with filters
    let (cdrs, total) = repo
        .list_filtered(
            query.account_id,
            query.caller.as_deref(),
            query.callee.as_deref(),
            start_date,
            end_date,
            &CdrListFilter {
                direction: query.direction.map(|d| d.as_str().to_string()),
                hangup_cause: query.hangup_cause.clone(),
                answered_only: query.answered_only,
            },
            query.pagination.limit(),
            query.pagination.offset(),
        )
        .await?;

    let response_data: Vec<CdrResponse> = cdrs.into_iter().map(CdrResponse::from).collect();

    info!(
        "Retrieved {} CDRs out of {} total",
        response_data.len(),
        total
    );

    Ok(Json(query.pagination.paginate(response_data, total)))
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
    _user: AuthenticatedUser,
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

    Ok(Json(ApiResponse::success(CdrResponse::from(cdr))))
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
        return Err(AppError::Validation(
            "end_date es anterior a start_date".to_string(),
        )
        .into());
    }
    if end - start > Duration::days(MAX_EXPORT_DAYS) + Duration::hours(1) {
        return Err(AppError::Validation(format!(
            "El export admite como máximo {} días por archivo",
            MAX_EXPORT_DAYS
        ))
        .into());
    }

    let mut params = query.into_inner();
    params.start_date = Some(start.to_rfc3339());
    params.end_date = Some(end.to_rfc3339());

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
            "start_date": params.start_date,
            "end_date": params.end_date,
            "direction": params.direction.map(|d| d.as_str()),
            "caller": params.caller,
            "callee": params.callee,
            "account_id": params.account_id,
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

    match format {
        ExportFormat::Csv => {
            let stream = create_csv_stream(pool, params).await?;
            Ok(response.streaming(stream))
        }
        ExportFormat::Json => {
            let stream = create_json_stream(pool, params).await?;
            Ok(response.streaming(stream))
        }
        ExportFormat::Jsonl => {
            let stream = create_jsonl_stream(pool, params).await?;
            Ok(response.streaming(stream))
        }
    }
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

/// Create CSV streaming response
async fn create_csv_stream(
    pool: PgPool,
    params: CdrExportParams,
) -> AppResult<impl futures::Stream<Item = Result<actix_web::web::Bytes, actix_web::Error>>> {
    const BATCH_SIZE: i64 = 1000;

    let stream = stream::unfold(
        (0_i64, false, pool, params),
        |(offset, done, pool, params)| async move {
            if done {
                return None;
            }

            // Fetch batch
            let repo = PgCdrRepository::new(pool.clone());
            let (cdrs, _) = match repo
                .list_filtered(
                    params.account_id,
                    params.caller.as_deref(),
                    params.callee.as_deref(),
                    parse_start_date(&params.start_date).unwrap_or(None),
                    parse_end_date(&params.end_date).unwrap_or(None),
                    &CdrListFilter {
                        direction: params.direction.map(|d| d.as_str().to_string()),
                        ..Default::default()
                    },
                    BATCH_SIZE,
                    offset,
                )
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    error!("Error fetching CDRs for export: {}", e);
                    return None;
                }
            };

            // An empty export still gets its header
            if cdrs.is_empty() && offset > 0 {
                return None;
            }

            let is_done = cdrs.len() < BATCH_SIZE as usize || offset + BATCH_SIZE >= params.limit;

            // Convert to CSV
            let mut csv_data = Vec::new();

            // Write header on first batch
            if offset == 0 {
                if let Err(e) = writeln!(&mut csv_data, "{}", CSV_HEADER) {
                    error!("Error writing CSV header: {}", e);
                    return None;
                }
            }

            // Write rows
            for cdr in cdrs {
                let lima_time = |dt: Option<DateTime<Utc>>| {
                    dt.map(|t| {
                        t.with_timezone(&chrono_tz::America::Lima)
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string()
                    })
                    .unwrap_or_default()
                };
                let fields = [
                    cdr.id.to_string(),
                    cdr.call_uuid.clone(),
                    cdr.account_id.map(|id| id.to_string()).unwrap_or_default(),
                    cdr.caller_number.clone(),
                    cdr.called_number.clone(),
                    cdr.destination_prefix.clone().unwrap_or_default(),
                    lima_time(Some(cdr.start_time)),
                    lima_time(cdr.answer_time),
                    lima_time(Some(cdr.end_time)),
                    cdr.duration.to_string(),
                    cdr.billsec.to_string(),
                    cdr.hangup_cause.clone(),
                    cdr.direction.clone(),
                ];
                let line = fields.iter().map(|f| csv_field(f)).collect::<Vec<_>>().join(",");
                if let Err(e) = writeln!(&mut csv_data, "{}", line) {
                    error!("Error writing CSV row: {}", e);
                    return None;
                }
            }

            Some((
                Ok(actix_web::web::Bytes::from(csv_data)),
                (offset + BATCH_SIZE, is_done, pool, params),
            ))
        },
    );

    Ok(stream)
}

/// Create JSON streaming response (array format)
async fn create_json_stream(
    pool: PgPool,
    params: CdrExportParams,
) -> AppResult<impl futures::Stream<Item = Result<actix_web::web::Bytes, actix_web::Error>>> {
    const BATCH_SIZE: i64 = 1000;

    let stream = stream::unfold(
        (0_i64, false, pool, params, true),
        |(offset, done, pool, params, is_first)| async move {
            if done {
                // Send closing bracket
                if !is_first {
                    return Some((
                        Ok(actix_web::web::Bytes::from("]")),
                        (offset, true, pool, params, false),
                    ));
                }
                return None;
            }

            // Fetch batch
            let repo = PgCdrRepository::new(pool.clone());
            let (cdrs, _) = match repo
                .list_filtered(
                    params.account_id,
                    params.caller.as_deref(),
                    params.callee.as_deref(),
                    parse_start_date(&params.start_date).unwrap_or(None),
                    parse_end_date(&params.end_date).unwrap_or(None),
                    &CdrListFilter {
                        direction: params.direction.map(|d| d.as_str().to_string()),
                        ..Default::default()
                    },
                    BATCH_SIZE,
                    offset,
                )
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    error!("Error fetching CDRs for export: {}", e);
                    return None;
                }
            };

            let mut json_data = String::new();

            // Opening bracket on first batch
            if is_first {
                json_data.push('[');
            }

            if cdrs.is_empty() {
                if is_first {
                    json_data.push(']');
                }
                return Some((
                    Ok(actix_web::web::Bytes::from(json_data)),
                    (offset, true, pool, params, false),
                ));
            }

            let is_done = cdrs.len() < BATCH_SIZE as usize || offset + BATCH_SIZE >= params.limit;

            // Write JSON objects
            for (i, cdr) in cdrs.iter().enumerate() {
                if !is_first || i > 0 {
                    json_data.push(',');
                }

                let row = CdrExportRow::from(cdr.clone());
                if let Ok(json) = serde_json::to_string(&row) {
                    json_data.push_str(&json);
                }
            }

            Some((
                Ok(actix_web::web::Bytes::from(json_data)),
                (offset + BATCH_SIZE, is_done, pool, params, false),
            ))
        },
    );

    Ok(stream)
}

/// Create JSON Lines streaming response (one object per line)
async fn create_jsonl_stream(
    pool: PgPool,
    params: CdrExportParams,
) -> AppResult<impl futures::Stream<Item = Result<actix_web::web::Bytes, actix_web::Error>>> {
    const BATCH_SIZE: i64 = 1000;

    let stream = stream::unfold(
        (0_i64, false, pool, params),
        |(offset, done, pool, params)| async move {
            if done {
                return None;
            }

            // Fetch batch
            let repo = PgCdrRepository::new(pool.clone());
            let (cdrs, _) = match repo
                .list_filtered(
                    params.account_id,
                    params.caller.as_deref(),
                    params.callee.as_deref(),
                    parse_start_date(&params.start_date).unwrap_or(None),
                    parse_end_date(&params.end_date).unwrap_or(None),
                    &CdrListFilter {
                        direction: params.direction.map(|d| d.as_str().to_string()),
                        ..Default::default()
                    },
                    BATCH_SIZE,
                    offset,
                )
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    error!("Error fetching CDRs for export: {}", e);
                    return None;
                }
            };

            if cdrs.is_empty() {
                return None;
            }

            let is_done = cdrs.len() < BATCH_SIZE as usize || offset + BATCH_SIZE >= params.limit;

            let mut jsonl_data = String::new();

            // Write JSON Lines
            for cdr in cdrs {
                let row = CdrExportRow::from(cdr);
                if let Ok(json) = serde_json::to_string(&row) {
                    jsonl_data.push_str(&json);
                    jsonl_data.push('\n');
                }
            }

            Some((
                Ok(actix_web::web::Bytes::from(jsonl_data)),
                (offset + BATCH_SIZE, is_done, pool, params),
            ))
        },
    );

    Ok(stream)
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
const CSV_HEADER: &str = "id,call_uuid,account_id,caller,callee,destination,start_time (GMT-5),answer_time (GMT-5),end_time (GMT-5),duration,billsec,hangup_cause,direction";

/// RFC 4180 field: quoted when it holds a comma, quote or line break; values a
/// spreadsheet would run as a formula (= + - @, tab, CR) are prefixed with '
fn csv_field(value: &str) -> String {
    let guarded = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
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
    fn test_csv_field_quotes_and_formula_guard() {
        assert_eq!(csv_field("912345678"), "912345678");
        assert_eq!(csv_field("+18505551234"), "'+18505551234");
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
            parse_start_date(&some("2026-10-24")).unwrap().unwrap().to_rfc3339(),
            "2026-10-24T05:00:00+00:00"
        );
        assert_eq!(
            parse_end_date(&some("2026-10-24")).unwrap().unwrap().to_rfc3339(),
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
