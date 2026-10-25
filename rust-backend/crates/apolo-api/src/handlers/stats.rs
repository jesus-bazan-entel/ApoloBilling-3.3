//! Statistics handlers
//!
//! NOC dashboard overview (traffic, quality, capacity) and its chart marks.

use crate::dto::{
    ApiResponse, NocCapacity, NocCause, NocEvent, NocEventRequest, NocKpis, NocOverviewQuery,
    NocOverviewResponse, NocQuality, NocRange, NocSlot, NocSlotValues, NocThresholds, NocTotals,
};
use actix_web::{web, HttpResponse};
use apolo_auth::{AdminUser, AuthenticatedUser};
use apolo_core::AppError;
use chrono::{DateTime, Duration, Utc};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use tracing::{info, instrument, warn};

// ============================================================================
// NOC overview
// ============================================================================

/// Local timezone of the operation (day boundaries, slot labels).
/// Override with DASHBOARD_TIMEZONE (an IANA name PostgreSQL knows).
fn dashboard_timezone() -> String {
    std::env::var("DASHBOARD_TIMEZONE").unwrap_or_else(|_| "America/Lima".to_string())
}

const SLOT_MINUTES: i32 = 15;
const SLOTS_PER_DAY: i32 = 24 * 60 / SLOT_MINUTES;

/// Q.850 causes that mean the network failed the call (ITU-T E.425 NER):
/// everything else unanswered is the user's choice (busy, rejected, cancel...).
const NETWORK_FAILURE_CAUSES: &[&str] = &[
    "RECOVERY_ON_TIMER_EXPIRE",
    "NORMAL_TEMPORARY_FAILURE",
    "NETWORK_OUT_OF_ORDER",
    "DESTINATION_OUT_OF_ORDER",
    "SWITCH_CONGESTION",
    "NORMAL_CIRCUIT_CONGESTION",
    "REQUESTED_CHAN_UNAVAIL",
    "NO_ROUTE_DESTINATION",
    "NO_ROUTE_TRANSIT_NET",
    "INCOMPATIBLE_DESTINATION",
    "MEDIA_TIMEOUT",
    "SERVICE_UNAVAILABLE",
    "BEARERCAPABILITY_NOTAVAIL",
    "INTERWORKING",
    "PROTOCOL_ERROR",
    "GATEWAY_DOWN",
    "MANDATORY_IE_MISSING",
    "INVALID_MSG_UNSPECIFIED",
];

const USER_CAUSES: &[&str] = &[
    // Unanswered NORMAL_CLEARING is a parallel-ring bridge whose answering leg
    // was busy: FreeSWITCH replies 486 but keeps Q.850 16 (all of a 40-call
    // sample on 2026-10-24 were "Overriding SIP cause 480 with 486").
    "NORMAL_CLEARING",
    "USER_BUSY",
    "NO_ANSWER",
    "NO_USER_RESPONSE",
    "CALL_REJECTED",
    "UNALLOCATED_NUMBER",
    "NUMBER_CHANGED",
    "SUBSCRIBER_ABSENT",
    "INVALID_NUMBER_FORMAT",
    "USER_NOT_REGISTERED",
];

fn cause_category(cause: &str) -> &'static str {
    if NETWORK_FAILURE_CAUSES.contains(&cause) {
        "network"
    } else if cause == "ORIGINATOR_CANCEL" {
        "cancel"
    } else if USER_CAUSES.contains(&cause) {
        "user"
    } else {
        "other"
    }
}

async fn noc_totals(
    pool: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<NocTotals, AppError> {
    let network: Vec<String> = NETWORK_FAILURE_CAUSES
        .iter()
        .map(|c| c.to_string())
        .collect();
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*)::BIGINT AS attempts,
            COUNT(answer_time)::BIGINT AS answered,
            COALESCE(SUM(billsec) FILTER (WHERE answer_time IS NOT NULL), 0)::BIGINT AS billsec,
            COUNT(*) FILTER (WHERE answer_time IS NULL AND hangup_cause = ANY($3))::BIGINT
                AS network_failures
        FROM cdrs
        WHERE start_time >= $1 AND start_time < $2
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(&network)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch NOC totals: {}", e)))?;

    Ok(NocTotals {
        attempts: row.get("attempts"),
        answered: row.get("answered"),
        billsec: row.get("billsec"),
        network_failures: row.get("network_failures"),
    })
}

/// Attempts / answered per 15-min slot and calls in progress at each slot's
/// midpoint, for the local day starting at `day_start`.
async fn noc_day_slots(
    pool: &PgPool,
    day_start: DateTime<Utc>,
) -> Result<Vec<NocSlotValues>, AppError> {
    let mut slots = vec![NocSlotValues::default(); SLOTS_PER_DAY as usize];

    let counts = sqlx::query(
        r#"
        SELECT
            FLOOR(EXTRACT(EPOCH FROM start_time - $1) / ($2 * 60))::INTEGER AS slot,
            COUNT(*)::BIGINT AS attempts,
            COUNT(answer_time)::BIGINT AS answered
        FROM cdrs
        WHERE start_time >= $1 AND start_time < $1 + INTERVAL '1 day'
        GROUP BY 1
        "#,
    )
    .bind(day_start)
    .bind(SLOT_MINUTES)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch NOC slots: {}", e)))?;

    for row in counts {
        let slot: i32 = row.get("slot");
        if let Some(s) = slots.get_mut(slot as usize) {
            s.attempts = row.get("attempts");
            s.answered = row.get("answered");
        }
    }

    // Calls are written at hangup, so the last minutes of today undercount
    // calls still in progress; the live counter comes from active_calls.
    let concurrent = sqlx::query(
        r#"
        SELECT g AS slot, (
            SELECT COUNT(*)::BIGINT FROM cdrs c
            WHERE c.start_time <= t
              AND c.start_time > t - INTERVAL '6 hours'
              AND COALESCE(c.end_time, c.start_time + make_interval(secs => c.duration)) > t
        ) AS concurrent
        FROM generate_series(0, $2 - 1) AS g,
             LATERAL (SELECT $1 + make_interval(mins => g * $3) + make_interval(secs => $3 * 30) AS t) x
        "#,
    )
    .bind(day_start)
    .bind(SLOTS_PER_DAY)
    .bind(SLOT_MINUTES)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch NOC concurrency: {}", e)))?;

    for row in concurrent {
        let slot: i32 = row.get("slot");
        if let Some(s) = slots.get_mut(slot as usize) {
            s.concurrent = row.get("concurrent");
        }
    }

    Ok(slots)
}

/// noc_* keys of system_settings (thresholds, capacity limits)
async fn noc_settings(pool: &PgPool) -> HashMap<String, String> {
    sqlx::query_as::<_, (String, String)>(
        r"SELECT key, value FROM system_settings WHERE key LIKE 'noc\_%'",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_else(|e| {
        warn!("NOC settings unavailable, using defaults: {}", e);
        Vec::new()
    })
    .into_iter()
    .collect()
}

fn setting<T: std::str::FromStr>(settings: &HashMap<String, String>, key: &str) -> Option<T> {
    settings.get(key).and_then(|v| v.trim().parse().ok())
}

fn noc_thresholds(settings: &HashMap<String, String>) -> NocThresholds {
    NocThresholds {
        asr_warn: setting(settings, "noc_asr_warn").unwrap_or(50.0),
        asr_crit: setting(settings, "noc_asr_crit").unwrap_or(40.0),
        ner_warn: setting(settings, "noc_ner_warn").unwrap_or(99.0),
        ner_crit: setting(settings, "noc_ner_crit").unwrap_or(97.0),
        min_range_attempts: setting(settings, "noc_min_range_attempts").unwrap_or(20),
    }
}

/// Call-rate headroom over the last hour. CDRs are written at hangup, so
/// calls still in progress are not counted yet.
async fn noc_capacity(
    pool: &PgPool,
    settings: &HashMap<String, String>,
) -> Result<NocCapacity, AppError> {
    let row = sqlx::query(
        r#"
        SELECT COALESCE(MAX(n), 0)::BIGINT AS peak, COALESCE(SUM(n), 0)::BIGINT AS total
        FROM (
            SELECT COUNT(*) AS n FROM cdrs
            WHERE start_time >= now() - INTERVAL '1 hour'
            GROUP BY date_trunc('second', start_time)
        ) s
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch NOC capacity: {}", e)))?;

    Ok(NocCapacity {
        max_channels: setting(settings, "noc_max_channels"),
        max_cps: setting(settings, "noc_max_cps"),
        peak_cps_last_hour: row.get("peak"),
        avg_cps_last_hour: row.get::<i64, _>("total") as f64 / 3600.0,
    })
}

/// RTPEngine media quality over the last 15 minutes of the monitoring
/// collector's 1-minute samples (the hourly rollup is only a fallback: it
/// lags up to two hours). MOS and jitter are RTPEngine's own running
/// averages; packet loss is the share of relayed packets lost in the window
/// (both are counters). Never fails the overview: the tables belong to the
/// collector.
async fn noc_quality(pool: &PgPool) -> NocQuality {
    let mut q = NocQuality {
        window_start: None,
        as_of: None,
        window_minutes: 15,
        mos: None,
        jitter_ms: None,
        packet_loss_pct: None,
        one_sided_sessions: None,
        active_sessions: None,
        pdd_p50_ms: None,
        pdd_p95_ms: None,
        pdd_samples: 0,
    };
    let loss = |lost: Option<f64>, sent: Option<f64>| match (lost, sent) {
        (Some(lost), Some(sent)) if sent > 0.0 => Some(100.0 * lost / sent),
        _ => None,
    };

    let live = sqlx::query(
        r#"
        SELECT MIN("timestamp") AT TIME ZONE 'UTC' AS window_start,
               MAX("timestamp") AT TIME ZONE 'UTC' AS as_of,
               (ARRAY_AGG(mos_avg ORDER BY "timestamp" DESC))[1] AS mos,
               (ARRAY_AGG(jitter_avg_ms ORDER BY "timestamp" DESC))[1] AS jitter,
               (MAX(packets_lost_in) - MIN(packets_lost_in))::FLOAT8 AS lost,
               (MAX(packets_in) - MIN(packets_in))::FLOAT8 AS sent,
               AVG(one_sided_sessions)::FLOAT8 AS one_sided,
               AVG(active_sessions)::FLOAT8 AS active
        FROM monitoring_rtpengine_metrics
        WHERE "timestamp" >= (now() AT TIME ZONE 'UTC') - INTERVAL '15 minutes'
        "#,
    )
    .fetch_one(pool)
    .await;

    match live {
        Ok(row) if row.get::<Option<DateTime<Utc>>, _>("as_of").is_some() => {
            q.window_start = row.get("window_start");
            q.as_of = row.get("as_of");
            q.mos = row.get("mos");
            q.jitter_ms = row.get("jitter");
            q.packet_loss_pct = loss(row.get("lost"), row.get("sent"));
            q.one_sided_sessions = row.get("one_sided");
            q.active_sessions = row.get("active");
        }
        other => {
            if let Err(e) = other {
                warn!("NOC live quality unavailable, using hourly rollup: {}", e);
            }
            let rows = sqlx::query(
                r#"
                SELECT metric, avg_value, max_value - min_value AS delta,
                       bucket_start AT TIME ZONE 'UTC' AS bucket_start
                FROM monitoring_kpi_hourly
                WHERE component = 'rtpengine'
                  AND bucket_start = (SELECT MAX(bucket_start) FROM monitoring_kpi_hourly
                                      WHERE component = 'rtpengine')
                  AND metric IN ('mos_avg', 'jitter_avg_ms', 'packets_lost_in', 'packets_in',
                                 'one_sided_sessions', 'active_sessions')
                "#,
            )
            .fetch_all(pool)
            .await
            .unwrap_or_else(|e| {
                warn!("NOC quality unavailable: {}", e);
                Vec::new()
            });
            let mut avg = HashMap::new();
            let mut delta = HashMap::new();
            for row in rows {
                let metric: String = row.get("metric");
                q.window_start = row.get("bucket_start");
                avg.insert(metric.clone(), row.get::<Option<f64>, _>("avg_value"));
                delta.insert(metric, row.get::<Option<f64>, _>("delta"));
            }
            let get = |m: &HashMap<String, Option<f64>>, k: &str| m.get(k).copied().flatten();
            q.window_minutes = 60;
            q.as_of = q.window_start.map(|t| t + Duration::hours(1));
            q.mos = get(&avg, "mos_avg");
            q.jitter_ms = get(&avg, "jitter_avg_ms");
            q.packet_loss_pct = loss(get(&delta, "packets_lost_in"), get(&delta, "packets_in"));
            q.one_sided_sessions = get(&avg, "one_sided_sessions");
            q.active_sessions = get(&avg, "active_sessions");
        }
    }
    // 0 means RTPEngine had no samples yet, not a MOS of zero
    q.mos = q.mos.filter(|v| *v > 0.0);

    match sqlx::query(
        r#"
        SELECT PERCENTILE_DISC(0.5) WITHIN GROUP (ORDER BY pdd_ms) AS p50,
               PERCENTILE_DISC(0.95) WITHIN GROUP (ORDER BY pdd_ms) AS p95,
               COUNT(pdd_ms)::BIGINT AS samples
        FROM cdrs
        WHERE start_time >= now() - INTERVAL '1 hour' AND pdd_ms IS NOT NULL
        "#,
    )
    .fetch_one(pool)
    .await
    {
        Ok(row) => {
            q.pdd_p50_ms = row.get("p50");
            q.pdd_p95_ms = row.get("p95");
            q.pdd_samples = row.get("samples");
        }
        Err(e) => warn!("NOC PDD unavailable (migration 020 applied?): {}", e),
    }
    q
}

/// Annotations overlapping [from, to). Never fails the overview.
async fn noc_events(pool: &PgPool, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<NocEvent> {
    sqlx::query(
        r#"
        SELECT id, kind, title, starts_at, ends_at, created_by
        FROM noc_events
        WHERE starts_at < $2 AND COALESCE(ends_at, starts_at) >= $1
        ORDER BY starts_at
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await
    .unwrap_or_else(|e| {
        warn!("NOC events unavailable (migration 019 applied?): {}", e);
        Vec::new()
    })
    .into_iter()
    .map(|row| NocEvent {
        id: row.get("id"),
        kind: row.get("kind"),
        title: row.get("title"),
        starts_at: row.get("starts_at"),
        ends_at: row.get("ends_at"),
        created_by: row.get("created_by"),
    })
    .collect()
}

/// Carrier NOC overview: today vs a comparison day (yesterday or the same
/// weekday last week), in the operation's local time.
///
/// GET /api/v1/stats/noc?compare=yesterday|week
#[instrument(skip(pool, _user))]
pub async fn get_noc_overview(
    pool: web::Data<PgPool>,
    query: web::Query<NocOverviewQuery>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    let pool = pool.get_ref();
    let tz = dashboard_timezone();
    let (compare, compare_days) = match query.compare.as_deref() {
        Some("week") => ("week", 7),
        _ => ("yesterday", 1),
    };

    let bounds = sqlx::query(
        r#"
        SELECT
            now() AS now_ts,
            (date_trunc('day', now() AT TIME ZONE $1) AT TIME ZONE $1) AS day_start,
            (date_trunc('day', (now() AT TIME ZONE $1) - make_interval(days => $2)) AT TIME ZONE $1)
                AS compare_start
        "#,
    )
    .bind(&tz)
    .bind(compare_days)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to resolve NOC day bounds: {}", e)))?;

    let now: DateTime<Utc> = bounds.get("now_ts");
    let day_start: DateTime<Utc> = bounds.get("day_start");
    let compare_start: DateTime<Utc> = bounds.get("compare_start");
    let elapsed = now - day_start;
    let compare_same_time_end = compare_start + elapsed;

    let kpis = NocKpis {
        today: noc_totals(pool, day_start, now).await?,
        compare_same_time: noc_totals(pool, compare_start, compare_same_time_end).await?,
        compare_full_day: noc_totals(pool, compare_start, compare_start + Duration::days(1))
            .await?,
    };

    let current_slot = (elapsed.num_minutes() / SLOT_MINUTES as i64) as i32;
    let today_slots = noc_day_slots(pool, day_start).await?;
    let compare_slots = noc_day_slots(pool, compare_start).await?;
    let series = (0..SLOTS_PER_DAY)
        .map(|i| {
            let minutes = i * SLOT_MINUTES;
            NocSlot {
                slot: i,
                label: format!("{:02}:{:02}", minutes / 60, minutes % 60),
                today: (i < current_slot).then(|| today_slots[i as usize].clone()),
                compare: compare_slots[i as usize].clone(),
            }
        })
        .collect();

    let causes = sqlx::query(
        r#"
        SELECT
            hangup_cause AS cause,
            COUNT(*) FILTER (WHERE start_time >= $1 AND start_time < $2)::BIGINT AS today,
            COUNT(*) FILTER (WHERE start_time >= $3 AND start_time < $4)::BIGINT AS compare
        FROM cdrs
        WHERE answer_time IS NULL
          AND hangup_cause IS NOT NULL
          AND ((start_time >= $1 AND start_time < $2) OR (start_time >= $3 AND start_time < $4))
        GROUP BY hangup_cause
        ORDER BY today DESC, compare DESC
        LIMIT 10
        "#,
    )
    .bind(day_start)
    .bind(now)
    .bind(compare_start)
    .bind(compare_same_time_end)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch NOC causes: {}", e)))?
    .into_iter()
    .map(|row| {
        let cause: String = row.get("cause");
        NocCause {
            category: cause_category(&cause).to_string(),
            cause,
            today: row.get("today"),
            compare: row.get("compare"),
        }
    })
    .collect();

    let ranges = sqlx::query(
        r#"
        SELECT
            LEFT(called_number, 4) AS prefix,
            CASE WHEN freeswitch_server_id = 'kamailio' THEN 'direct' ELSE 'freeswitch' END AS route,
            COUNT(*) FILTER (WHERE start_time >= $1)::BIGINT AS attempts,
            COUNT(answer_time) FILTER (WHERE start_time >= $1)::BIGINT AS answered,
            COALESCE(SUM(billsec) FILTER (WHERE start_time >= $1 AND answer_time IS NOT NULL), 0)::BIGINT
                AS billsec,
            COUNT(*) FILTER (WHERE start_time < $4)::BIGINT AS compare_attempts
        FROM cdrs
        WHERE (start_time >= $1 AND start_time < $2) OR (start_time >= $3 AND start_time < $4)
        GROUP BY 1, 2
        HAVING COUNT(*) FILTER (WHERE start_time >= $1) > 0
        ORDER BY attempts DESC
        LIMIT 25
        "#,
    )
    .bind(day_start)
    .bind(now)
    .bind(compare_start)
    .bind(compare_same_time_end)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch NOC ranges: {}", e)))?
    .into_iter()
    .map(|row| NocRange {
        prefix: row.get("prefix"),
        route: row.get("route"),
        attempts: row.get("attempts"),
        answered: row.get("answered"),
        billsec: row.get("billsec"),
        compare_attempts: row.get("compare_attempts"),
        hourly_attempts: vec![0; 24],
    })
    .collect::<Vec<_>>();

    let mut ranges = ranges;
    let hourly = sqlx::query(
        r#"
        SELECT
            LEFT(called_number, 4) AS prefix,
            CASE WHEN freeswitch_server_id = 'kamailio' THEN 'direct' ELSE 'freeswitch' END AS route,
            FLOOR(EXTRACT(EPOCH FROM start_time - $1) / 3600)::INTEGER AS hour,
            COUNT(*)::BIGINT AS attempts
        FROM cdrs
        WHERE start_time >= $1 AND start_time < $2
        GROUP BY 1, 2, 3
        "#,
    )
    .bind(day_start)
    .bind(now)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch NOC range trend: {}", e)))?;
    for row in hourly {
        let prefix: Option<String> = row.get("prefix");
        let route: String = row.get("route");
        let hour: i32 = row.get("hour");
        if let Some(r) = ranges
            .iter_mut()
            .find(|r| r.prefix == prefix.as_deref().unwrap_or_default() && r.route == route)
        {
            if let Some(v) = r.hourly_attempts.get_mut(hour as usize) {
                *v = row.get("attempts");
            }
        }
    }

    let settings = noc_settings(pool).await;
    let thresholds = noc_thresholds(&settings);
    let capacity = noc_capacity(pool, &settings).await?;
    let quality = noc_quality(pool).await;
    let events = noc_events(
        pool,
        compare_start.min(day_start),
        compare_start.max(day_start) + Duration::days(1),
    )
    .await;

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(NocOverviewResponse {
            timezone: tz,
            compare: compare.to_string(),
            day_start,
            compare_day_start: compare_start,
            generated_at: now,
            current_slot,
            slot_minutes: SLOT_MINUTES,
            kpis,
            series,
            causes,
            ranges,
            thresholds,
            capacity,
            quality,
            events,
        })),
    )
}

const NOC_EVENT_KINDS: &[&str] = &["incident", "campaign", "maintenance", "migration", "other"];

/// Add a chart annotation
///
/// POST /api/v1/stats/noc/events
#[instrument(skip(pool, user, body))]
pub async fn create_noc_event(
    pool: web::Data<PgPool>,
    body: web::Json<NocEventRequest>,
    user: AdminUser,
) -> Result<HttpResponse, AppError> {
    let title = body.title.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return Err(AppError::Validation(
            "El título es obligatorio (máx. 120 caracteres)".to_string(),
        ));
    }
    if !NOC_EVENT_KINDS.contains(&body.kind.as_str()) {
        return Err(AppError::Validation(format!(
            "Tipo de marca inválido: {}",
            body.kind
        )));
    }
    if body.ends_at.is_some_and(|end| end < body.starts_at) {
        return Err(AppError::Validation(
            "El fin no puede ser anterior al inicio".to_string(),
        ));
    }

    let row = sqlx::query(
        r#"
        INSERT INTO noc_events (kind, title, starts_at, ends_at, created_by)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, kind, title, starts_at, ends_at, created_by
        "#,
    )
    .bind(&body.kind)
    .bind(title)
    .bind(body.starts_at)
    .bind(body.ends_at)
    .bind(&user.username)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to create NOC event: {}", e)))?;

    info!(user = %user.username, title = %title, "NOC event created");

    Ok(HttpResponse::Created().json(ApiResponse::success(NocEvent {
        id: row.get("id"),
        kind: row.get("kind"),
        title: row.get("title"),
        starts_at: row.get("starts_at"),
        ends_at: row.get("ends_at"),
        created_by: row.get("created_by"),
    })))
}

/// Remove a chart annotation
///
/// DELETE /api/v1/stats/noc/events/{id}
#[instrument(skip(pool, user))]
pub async fn delete_noc_event(
    pool: web::Data<PgPool>,
    path: web::Path<i64>,
    user: AdminUser,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let result = sqlx::query("DELETE FROM noc_events WHERE id = $1")
        .bind(id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete NOC event: {}", e)))?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Marca {} no encontrada", id)));
    }
    info!(user = %user.username, id = id, "NOC event deleted");
    Ok(HttpResponse::NoContent().finish())
}

/// Configure statistics routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/stats")
            .route("/noc", web::get().to(get_noc_overview))
            .route("/noc/events", web::post().to(create_noc_event))
            .route("/noc/events/{id}", web::delete().to(delete_noc_event)),
    );
}
