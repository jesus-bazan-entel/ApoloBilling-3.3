//! Endpoint Monitor Handlers
//!
//! HTTP handlers for the SIP endpoint connectivity monitoring system.
//! Provides endpoints for viewing probe status, historical data,
//! and downloading evidence traces.

use crate::dto::endpoint_monitor::*;
use crate::dto::ApiResponse;
use actix_web::{http::header, web, HttpResponse};
use apolo_auth::AuthenticatedUser;
use apolo_core::AppError;
use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use tracing::{error, info, instrument};
use uuid::Uuid;

// ============================================================================
// Endpoint Status Handler
// ============================================================================

/// Get the latest probe status for all endpoints
///
/// GET /api/v1/monitor/endpoints
///
/// Returns the status from the most recent probe cycle for all endpoints.
/// Used for the dashboard cards.
#[instrument(skip(pool, user))]
pub async fn get_endpoint_status(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("endpoint_monitor") {
        return Err(AppError::Forbidden);
    }

    info!("Getting endpoint monitor status");

    // Get the maximum timestamp (latest probe cycle)
    let max_ts_row = sqlx::query("SELECT MAX(ts) as max_ts FROM endpoint_monitor_probes")
        .fetch_one(pool.get_ref())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get max timestamp");
            AppError::Database(format!("Failed to get max timestamp: {}", e))
        })?;

    let max_ts: Option<i64> = max_ts_row.try_get("max_ts").ok();

    let max_ts = match max_ts {
        Some(ts) => ts,
        None => {
            // No probes yet
            return Ok(
                HttpResponse::Ok().json(ApiResponse::success(EndpointStatusResponse {
                    endpoints: vec![],
                    generated_at: Utc::now().timestamp(),
                })),
            );
        }
    };

    // Get all probes from the latest cycle
    let rows = sqlx::query(
        r#"
        SELECT
            trunk_id,
            trunk_name,
            ip,
            port,
            trunk_type,
            status,
            resp_code,
            rtt_ms,
            ts
        FROM endpoint_monitor_probes
        WHERE ts = $1
        ORDER BY status ASC, trunk_name ASC
        "#,
    )
    .bind(max_ts)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to fetch endpoint status");
        AppError::Database(format!("Failed to fetch endpoint status: {}", e))
    })?;

    let endpoints: Vec<EndpointStatus> = rows
        .into_iter()
        .map(|r| EndpointStatus {
            trunk_id: r.get("trunk_id"),
            name: r.get("trunk_name"),
            ip: r.get("ip"),
            port: r.get("port"),
            trunk_type: r.get("trunk_type"),
            status: r.get("status"),
            resp_code: r.get("resp_code"),
            rtt_ms: r.get("rtt_ms"),
            last_ts: r.get("ts"),
        })
        .collect();

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(EndpointStatusResponse {
            endpoints,
            generated_at: Utc::now().timestamp(),
        })),
    )
}

// ============================================================================
// Probe Results Handler
// ============================================================================

/// Get historical probe results with downsampled series
///
/// GET /api/v1/monitor/results
///
/// Returns both a downsampled time series for charting and raw rows for the table.
#[instrument(skip(pool, user))]
pub async fn get_probe_results(
    pool: web::Data<PgPool>,
    query: web::Query<ProbeResultsQuery>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("endpoint_monitor") {
        return Err(AppError::Forbidden);
    }

    let frm = query.from_ts();
    let to = query.to_ts();
    let bucket = query.bucket_seconds();
    let limit = query.get_limit();

    info!(
        frm = frm,
        to = to,
        bucket = bucket,
        limit = limit,
        "Getting probe results"
    );

    // Build name filter for LIKE queries
    let name_filter = query.name.as_ref().map(|n| format!("%{}%", n));
    let ip_filter = query.ip.as_ref().map(|i| format!("%{}%", i));

    // Get bucketed series data
    let series_rows = sqlx::query(
        r#"
        SELECT
            trunk_id,
            trunk_name,
            ip,
            (FLOOR(ts::float / $3) * $3)::bigint AS tb,
            ROUND(AVG(rtt_ms), 3) AS rtt,
            MIN(status) AS status,
            SUM(CASE WHEN status = 'down' THEN 1 ELSE 0 END)::bigint AS downs,
            COUNT(*)::bigint AS n
        FROM endpoint_monitor_probes
        WHERE ts >= $1 AND ts <= $2
          AND ($4::text IS NULL OR trunk_name ILIKE $4)
          AND ($5::text IS NULL OR ip ILIKE $5)
        GROUP BY trunk_id, trunk_name, ip, tb
        ORDER BY trunk_name, ip, tb
        "#,
    )
    .bind(frm)
    .bind(to)
    .bind(bucket)
    .bind(&name_filter)
    .bind(&ip_filter)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to fetch series data");
        AppError::Database(format!("Failed to fetch series data: {}", e))
    })?;

    // Group series by endpoint
    let mut series_map: HashMap<(Uuid, String, String), Vec<SeriesPoint>> = HashMap::new();
    for row in series_rows {
        let trunk_id: Uuid = row.get("trunk_id");
        let trunk_name: String = row.get("trunk_name");
        let ip: String = row.get("ip");
        let tb: i64 = row.get("tb");
        let rtt: Option<Decimal> = row.get("rtt");
        let status: String = row.get("status");
        let downs: i64 = row.get("downs");
        let n: i64 = row.get("n");

        let key = (trunk_id, trunk_name.clone(), ip.clone());
        let point = SeriesPoint {
            t: tb,
            rtt,
            status,
            downs: downs as i32,
            n: n as i32,
        };
        series_map.entry(key).or_default().push(point);
    }

    let series: Vec<EndpointSeries> = series_map
        .into_iter()
        .map(|((trunk_id, name, ip), points)| EndpointSeries {
            trunk_id,
            name,
            ip,
            points,
        })
        .collect();

    // Get raw rows for the table (without evidence)
    let table_rows = sqlx::query(
        r#"
        SELECT
            id,
            ts,
            trunk_id,
            trunk_name,
            ip,
            trunk_type,
            port,
            status,
            resp_code,
            rtt_ms
        FROM endpoint_monitor_probes
        WHERE ts >= $1 AND ts <= $2
          AND ($4::text IS NULL OR trunk_name ILIKE $4)
          AND ($5::text IS NULL OR ip ILIKE $5)
        ORDER BY ts DESC
        LIMIT $3
        "#,
    )
    .bind(frm)
    .bind(to)
    .bind(limit as i64)
    .bind(&name_filter)
    .bind(&ip_filter)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to fetch table rows");
        AppError::Database(format!("Failed to fetch table rows: {}", e))
    })?;

    let rows: Vec<ProbeRow> = table_rows
        .into_iter()
        .map(|r| ProbeRow {
            id: r.get("id"),
            ts: r.get("ts"),
            trunk_id: r.get("trunk_id"),
            name: r.get("trunk_name"),
            ip: r.get("ip"),
            trunk_type: r.get("trunk_type"),
            port: r.get("port"),
            status: r.get("status"),
            resp_code: r.get("resp_code"),
            rtt_ms: r.get("rtt_ms"),
        })
        .collect();

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(ProbeResultsResponse {
            frm,
            to,
            bucket_seconds: bucket,
            generated_at: Utc::now().timestamp(),
            series,
            rows,
        })),
    )
}

// ============================================================================
// Evidence Download Handler
// ============================================================================

/// Download evidence trace for a single probe
///
/// GET /api/v1/monitor/evidence?id=123
///
/// Returns a text file with the raw sipsak trace for the specified probe.
#[instrument(skip(pool, user))]
pub async fn download_evidence(
    pool: web::Data<PgPool>,
    query: web::Query<EvidenceQuery>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("endpoint_monitor") {
        return Err(AppError::Forbidden);
    }

    info!(probe_id = query.id, "Downloading evidence for probe");

    let record = sqlx::query(
        r#"
        SELECT
            id,
            ts,
            trunk_name,
            ip,
            port,
            status,
            resp_code,
            rtt_ms,
            evidence
        FROM endpoint_monitor_probes
        WHERE id = $1
        "#,
    )
    .bind(query.id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to fetch evidence");
        AppError::Database(format!("Failed to fetch evidence: {}", e))
    })?;

    let record = match record {
        Some(r) => r,
        None => return Err(AppError::NotFound(format!("Probe {} not found", query.id))),
    };

    let id: i64 = record.get("id");
    let ts: i64 = record.get("ts");
    let trunk_name: String = record.get("trunk_name");
    let ip: String = record.get("ip");
    let port: i16 = record.get("port");
    let status: String = record.get("status");
    let resp_code: i16 = record.get("resp_code");
    let rtt_ms: Option<Decimal> = record.get("rtt_ms");
    let evidence: Option<String> = record.get("evidence");

    // Format timestamp
    let dt = Utc.timestamp_opt(ts, 0).single().unwrap_or_else(Utc::now);
    let date_str = dt.format("%Y%m%d_%H%M%S").to_string();

    // Build filename: evidence_<endpoint>_<date>.txt
    let safe_name = trunk_name.replace(|c: char| !c.is_alphanumeric(), "_");
    let filename = format!("evidence_{}_{}.txt", safe_name, date_str);

    // Build content
    let rtt_str = rtt_ms
        .map(|r| format!("{} ms", r))
        .unwrap_or_else(|| "N/A".to_string());

    let content = format!(
        "=== SIP Endpoint Probe Evidence ===\n\
         Probe ID: {}\n\
         Endpoint: {}\n\
         IP:Port: {}:{}\n\
         Timestamp (UTC): {}\n\
         Status: {}\n\
         Response Code: {}\n\
         RTT: {}\n\
         \n\
         === Raw Trace ===\n\
         {}\n",
        id,
        trunk_name,
        ip,
        port,
        dt.format("%Y-%m-%d %H:%M:%S UTC"),
        status.to_uppercase(),
        resp_code,
        rtt_str,
        evidence.unwrap_or_else(|| "(No trace available)".to_string())
    );

    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "text/plain; charset=utf-8"))
        .insert_header((
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        ))
        .body(content))
}

// ============================================================================
// Evidence Report Handler
// ============================================================================

/// Download consolidated availability report
///
/// GET /api/v1/monitor/evidence_report
///
/// Returns a text file with availability statistics and optionally full traces.
#[instrument(skip(pool, user))]
pub async fn download_evidence_report(
    pool: web::Data<PgPool>,
    query: web::Query<EvidenceReportQuery>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("endpoint_monitor") {
        return Err(AppError::Forbidden);
    }

    let frm = query.from_ts();
    let to = query.to_ts();
    let include_full = query.include_full();

    info!(
        frm = frm,
        to = to,
        full = include_full,
        "Downloading evidence report"
    );

    let name_filter = query.name.as_ref().map(|n| format!("%{}%", n));
    let ip_filter = query.ip.as_ref().map(|i| format!("%{}%", i));

    // Get statistics
    let stats_row = sqlx::query(
        r#"
        SELECT
            COUNT(*)::bigint AS total_probes,
            SUM(CASE WHEN status = 'up' THEN 1 ELSE 0 END)::bigint AS up_count,
            SUM(CASE WHEN status = 'down' THEN 1 ELSE 0 END)::bigint AS down_count,
            MIN(rtt_ms) AS min_rtt,
            ROUND(AVG(rtt_ms), 3) AS avg_rtt,
            MAX(rtt_ms) AS max_rtt
        FROM endpoint_monitor_probes
        WHERE ts >= $1 AND ts <= $2
          AND ($3::text IS NULL OR trunk_name ILIKE $3)
          AND ($4::text IS NULL OR ip ILIKE $4)
        "#,
    )
    .bind(frm)
    .bind(to)
    .bind(&name_filter)
    .bind(&ip_filter)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to fetch report stats");
        AppError::Database(format!("Failed to fetch report stats: {}", e))
    })?;

    let total_probes: i64 = stats_row.get("total_probes");
    let up_count: i64 = stats_row.get("up_count");
    let down_count: i64 = stats_row.get("down_count");
    let min_rtt: Option<Decimal> = stats_row.get("min_rtt");
    let avg_rtt: Option<Decimal> = stats_row.get("avg_rtt");
    let max_rtt: Option<Decimal> = stats_row.get("max_rtt");

    // Format timestamps
    let frm_dt = Utc.timestamp_opt(frm, 0).single().unwrap_or_else(Utc::now);
    let to_dt = Utc.timestamp_opt(to, 0).single().unwrap_or_else(Utc::now);
    let now_dt = Utc::now();

    // Calculate availability
    let availability = if total_probes > 0 {
        (up_count as f64 / total_probes as f64) * 100.0
    } else {
        0.0
    };

    // Build report header
    let mut content = format!(
        "=== SIP Endpoint Connectivity Report ===\n\
         Generated: {}\n\
         \n\
         === Report Parameters ===\n\
         From: {}\n\
         To: {}\n\
         Name filter: {}\n\
         IP filter: {}\n\
         \n\
         === Summary Statistics ===\n\
         Total Probes: {}\n\
         Up: {} ({:.2}%)\n\
         Down: {} ({:.2}%)\n\
         Availability: {:.2}%\n\
         \n\
         === RTT Statistics ===\n\
         Min: {} ms\n\
         Avg: {} ms\n\
         Max: {} ms\n",
        now_dt.format("%Y-%m-%d %H:%M:%S UTC"),
        frm_dt.format("%Y-%m-%d %H:%M:%S UTC"),
        to_dt.format("%Y-%m-%d %H:%M:%S UTC"),
        query.name.as_deref().unwrap_or("(all)"),
        query.ip.as_deref().unwrap_or("(all)"),
        total_probes,
        up_count,
        if total_probes > 0 {
            (up_count as f64 / total_probes as f64) * 100.0
        } else {
            0.0
        },
        down_count,
        if total_probes > 0 {
            (down_count as f64 / total_probes as f64) * 100.0
        } else {
            0.0
        },
        availability,
        min_rtt
            .map(|r| r.to_string())
            .unwrap_or_else(|| "N/A".to_string()),
        avg_rtt
            .map(|r| r.to_string())
            .unwrap_or_else(|| "N/A".to_string()),
        max_rtt
            .map(|r| r.to_string())
            .unwrap_or_else(|| "N/A".to_string()),
    );

    // Optionally add full traces (only for down probes to save space)
    if include_full {
        content.push_str("\n=== Failure Traces ===\n");

        let down_probes = sqlx::query(
            r#"
            SELECT
                id,
                ts,
                trunk_name,
                ip,
                port,
                status,
                resp_code,
                rtt_ms,
                evidence
            FROM endpoint_monitor_probes
            WHERE ts >= $1 AND ts <= $2
              AND status = 'down'
              AND ($3::text IS NULL OR trunk_name ILIKE $3)
              AND ($4::text IS NULL OR ip ILIKE $4)
            ORDER BY ts DESC
            LIMIT 100
            "#,
        )
        .bind(frm)
        .bind(to)
        .bind(&name_filter)
        .bind(&ip_filter)
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch down probes");
            AppError::Database(format!("Failed to fetch down probes: {}", e))
        })?;

        if down_probes.is_empty() {
            content.push_str("(No failures in this period)\n");
        } else {
            for probe in down_probes {
                let ts: i64 = probe.get("ts");
                let trunk_name: String = probe.get("trunk_name");
                let evidence: Option<String> = probe.get("evidence");

                let dt = Utc.timestamp_opt(ts, 0).single().unwrap_or_else(Utc::now);
                content.push_str(&format!(
                    "\n--- {} @ {} ---\n{}\n",
                    trunk_name,
                    dt.format("%Y-%m-%d %H:%M:%S UTC"),
                    evidence.unwrap_or_else(|| "(No trace)".to_string())
                ));
            }
        }
    }

    // Generate filename
    let date_str = now_dt.format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("endpoint_report_{}.txt", date_str);

    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "text/plain; charset=utf-8"))
        .insert_header((
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        ))
        .body(content))
}

// ============================================================================
// Route Configuration
// ============================================================================

/// Configure endpoint monitor routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/monitor")
            .route("/endpoints", web::get().to(get_endpoint_status))
            .route("/results", web::get().to(get_probe_results))
            .route("/evidence", web::get().to(download_evidence))
            .route("/evidence_report", web::get().to(download_evidence_report)),
    );
}
