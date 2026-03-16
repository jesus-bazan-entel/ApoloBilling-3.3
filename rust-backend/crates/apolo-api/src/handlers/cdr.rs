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
    models::Cdr,
    traits::{CdrRepository, PaginatedResponse, Repository},
    AppResult,
};
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

    // Parse date filters
    let start_date = parse_start_date(&query.start_date);
    let end_date = parse_end_date(&query.end_date);

    // Query with filters
    let (cdrs, total) = repo
        .list_filtered(
            query.account_id,
            query.caller.as_deref(),
            query.callee.as_deref(),
            start_date,
            end_date,
            query.pagination.limit(),
            query.pagination.offset(),
        )
        .await?;

    // Apply additional filters in memory (direction, answered_only)
    let mut filtered_cdrs: Vec<Cdr> = cdrs;

    if let Some(direction) = query.direction {
        filtered_cdrs.retain(|cdr| cdr.direction.to_lowercase() == direction.as_str());
    }

    if query.answered_only {
        filtered_cdrs.retain(|cdr| cdr.was_answered());
    }

    if let Some(ref cause) = query.hangup_cause {
        let cause_upper = cause.to_uppercase();
        filtered_cdrs.retain(|cdr| cdr.hangup_cause.to_uppercase().contains(&cause_upper));
    }

    let response_data: Vec<CdrResponse> =
        filtered_cdrs.into_iter().map(CdrResponse::from).collect();

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
pub async fn get_cdr(path: Path<i64>, db: Data<PgPool>) -> Result<Json<ApiResponse<CdrResponse>>> {
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
pub async fn export_cdrs(query: Query<CdrExportParams>, db: Data<PgPool>) -> Result<HttpResponse> {
    // Validate query parameters
    query.validate().map_err(|e| {
        warn!("Invalid export parameters: {}", e);
        AppError::Validation(e.to_string())
    })?;

    info!(
        "Starting CDR export: format={:?}, limit={}, account_id={:?}",
        query.format, query.limit, query.account_id
    );

    let format = query.format;
    let pool = db.get_ref().clone();

    // Build filename
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("cdrs_export_{}.{}", timestamp, format.extension());

    // Start streaming response
    let mut response = HttpResponse::Ok();
    response.content_type(format.content_type()).insert_header((
        "Content-Disposition",
        format!("attachment; filename=\"{}\"", filename),
    ));

    match format {
        ExportFormat::Csv => {
            let stream = create_csv_stream(pool, query.into_inner()).await?;
            Ok(response.streaming(stream))
        }
        ExportFormat::Json => {
            let stream = create_json_stream(pool, query.into_inner()).await?;
            Ok(response.streaming(stream))
        }
        ExportFormat::Jsonl => {
            let stream = create_jsonl_stream(pool, query.into_inner()).await?;
            Ok(response.streaming(stream))
        }
    }
}

/// Get CDR statistics
///
/// Calculates aggregated statistics including call counts, ASR, costs, and durations.
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
        "Calculated stats: total_calls={}, asr={}%, total_cost={}",
        result.total_calls, result.asr, result.total_cost
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
                    params.start_date,
                    params.end_date,
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

            // Convert to CSV
            let mut csv_data = Vec::new();

            // Write header on first batch
            if offset == 0 {
                if let Err(e) = writeln!(
                    &mut csv_data,
                    "id,call_uuid,account_id,caller,callee,destination,start_time,answer_time,end_time,duration,billsec,rate,cost,hangup_cause,direction"
                ) {
                    error!("Error writing CSV header: {}", e);
                    return None;
                }
            }

            // Write rows
            for cdr in cdrs {
                let row = CdrExportRow::from(cdr);
                if let Err(e) = writeln!(
                    &mut csv_data,
                    "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                    row.id,
                    row.call_uuid,
                    row.account_id,
                    row.caller,
                    row.callee,
                    row.destination,
                    row.start_time,
                    row.answer_time,
                    row.end_time,
                    row.duration,
                    row.billsec,
                    row.rate,
                    row.cost,
                    row.hangup_cause,
                    row.direction
                ) {
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
                    params.start_date,
                    params.end_date,
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
                    params.start_date,
                    params.end_date,
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
            total_cost: Decimal::ZERO,
            avg_cost: Decimal::ZERO,
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

    let total_cost: Decimal = cdrs.iter().filter_map(|cdr| cdr.cost).sum();

    let avg_duration = Decimal::from(total_duration) / Decimal::from(total_calls);
    let avg_billsec = Decimal::from(total_billsec) / Decimal::from(total_calls);
    let avg_cost = total_cost / Decimal::from(total_calls);

    CdrStats {
        total_calls,
        answered_calls,
        failed_calls,
        asr,
        total_duration,
        total_billsec,
        avg_duration,
        avg_billsec,
        total_cost,
        avg_cost,
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
            let cost: Decimal = cdrs.iter().filter_map(|cdr| cdr.cost).sum();

            TimeSeriesPoint {
                period,
                timestamp,
                calls,
                cost,
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

/// Parse optional date string to DateTime<Utc>
/// Parse start date (beginning of day in Lima timezone)
fn parse_start_date(date_str: &Option<String>) -> Option<DateTime<Utc>> {
    use chrono_tz::America::Lima;

    let s = date_str.as_ref()?;
    if s.is_empty() {
        return None;
    }

    // Try parsing as ISO 8601 (RFC 3339)
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    // Try parsing as simple date (start of day in Lima timezone)
    if let Ok(naive) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        if let Some(naive_dt) = naive.and_hms_opt(0, 0, 0) {
            // Parse date as Lima timezone, then convert to UTC for PostgreSQL
            let lima_dt = Lima.from_local_datetime(&naive_dt).unwrap();
            return Some(lima_dt.with_timezone(&Utc));
        }
    }

    None
}

/// Parse end date (end of day in Lima timezone - 23:59:59)
fn parse_end_date(date_str: &Option<String>) -> Option<DateTime<Utc>> {
    use chrono_tz::America::Lima;

    let s = date_str.as_ref()?;
    if s.is_empty() {
        return None;
    }

    // Try parsing as ISO 8601 (RFC 3339)
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    // Try parsing as simple date (END of day in Lima timezone - 23:59:59)
    if let Ok(naive) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        if let Some(naive_dt) = naive.and_hms_opt(23, 59, 59) {
            // Parse date as Lima timezone, then convert to UTC for PostgreSQL
            let lima_dt = Lima.from_local_datetime(&naive_dt).unwrap();
            return Some(lima_dt.with_timezone(&Utc));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use apolo_core::models::Cdr;

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
        cdr1.cost = Some(Decimal::new(100, 2)); // 1.00

        let mut cdr2 = Cdr::default();
        cdr2.duration = 30;
        cdr2.billsec = 0;
        cdr2.answer_time = None;
        cdr2.cost = Some(Decimal::ZERO);

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
