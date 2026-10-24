//! Statistics handlers
//!
//! HTTP handlers for dashboard statistics endpoints.

use crate::dto::{
    ApiResponse, BalanceTrendPoint, BalanceTrendResponse, CallsByHourResponse,
    CallsByTypeResponse, CallsByZoneResponse, CallTypeStats, DailyRevenueStats,
    HourlyCallStats, NocCause, NocKpis, NocOverviewQuery, NocOverviewResponse, NocRange, NocSlot,
    NocSlotValues, NocTotals, RevenueByDayResponse, TrafficByDirectionResponse, TrafficStats,
    ZoneStats,
};
use actix_web::{web, HttpResponse};
use apolo_auth::AuthenticatedUser;
use apolo_core::AppError;
use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc, Weekday};
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use tracing::{debug, instrument};

/// Get calls by hour (last 24 hours)
///
/// GET /api/v1/stats/calls-by-hour
#[instrument(skip(pool, _user))]
pub async fn get_calls_by_hour(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    debug!("Fetching hourly call statistics");

    // "Hoy" = since local midnight. The previous last-24h window folded
    // yesterday's later hours onto today's hour-of-day buckets.
    let rows = sqlx::query(
        r#"
        SELECT
            EXTRACT(HOUR FROM start_time AT TIME ZONE $1)::INTEGER as hour,
            COUNT(*)::BIGINT as call_count,
            COALESCE(SUM(duration), 0)::BIGINT as total_duration,
            COALESCE(SUM(cost), 0) as total_revenue
        FROM cdrs
        WHERE start_time >= (date_trunc('day', now() AT TIME ZONE $1) AT TIME ZONE $1)
        GROUP BY 1
        ORDER BY hour
        "#,
    )
    .bind(dashboard_timezone())
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch hourly stats: {}", e)))?;

    // Create a map of existing data
    let mut hour_map = std::collections::HashMap::new();
    for row in rows {
        let hour: i32 = row.get("hour");
        hour_map.insert(
            hour,
            HourlyCallStats {
                hour,
                hour_label: format!("{}:00", hour),
                call_count: row.get("call_count"),
                total_duration: row.get("total_duration"),
                total_revenue: row.get::<Decimal, _>("total_revenue"),
            },
        );
    }

    // Fill in all 24 hours (0-23) with zeros for missing hours
    let mut data: Vec<HourlyCallStats> = (0..24)
        .map(|hour| {
            hour_map.get(&hour).cloned().unwrap_or_else(|| {
                HourlyCallStats {
                    hour,
                    hour_label: format!("{}:00", hour),
                    call_count: 0,
                    total_duration: 0,
                    total_revenue: Decimal::ZERO,
                }
            })
        })
        .collect();

    // Sort by hour
    data.sort_by_key(|s| s.hour);

    Ok(HttpResponse::Ok().json(ApiResponse::success(CallsByHourResponse { data })))
}

/// Get revenue by day (last 7 days)
///
/// GET /api/v1/stats/revenue-by-day
#[instrument(skip(pool, _user))]
pub async fn get_revenue_by_day(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    debug!("Fetching daily revenue statistics");

    let now = Utc::now();
    let start_date = (now - Duration::days(7)).date_naive();

    let rows = sqlx::query(
        r#"
        SELECT
            DATE(start_time) as date,
            COUNT(*)::BIGINT as call_count,
            COALESCE(SUM(cost), 0) as revenue,
            COALESCE(SUM(billsec) / 60.0, 0) as total_minutes
        FROM cdrs
        WHERE DATE(start_time) >= $1
        GROUP BY DATE(start_time)
        ORDER BY date
        "#,
    )
    .bind(start_date)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch daily revenue: {}", e)))?;

    // Spanish day labels
    let day_labels = ["Dom", "Lun", "Mar", "Mié", "Jue", "Vie", "Sáb"];

    let mut data: Vec<DailyRevenueStats> = Vec::new();
    for row in rows {
        let date: chrono::NaiveDate = row.get("date");
        let weekday = date.weekday().num_days_from_sunday() as usize;

        data.push(DailyRevenueStats {
            date: date.format("%Y-%m-%d").to_string(),
            day_of_week: weekday as i32,
            day_label: day_labels[weekday].to_string(),
            call_count: row.get("call_count"),
            revenue: row.get::<Decimal, _>("revenue"),
            total_minutes: row.get::<Decimal, _>("total_minutes"),
        });
    }

    // Fill in missing days with zero data
    let mut all_days = Vec::new();
    for i in 0..7 {
        let day = (now - Duration::days(6 - i)).date_naive();
        let weekday = day.weekday().num_days_from_sunday() as usize;

        // Find existing data or create zero entry
        let existing = data.iter().find(|d| d.date == day.format("%Y-%m-%d").to_string());

        if let Some(existing_data) = existing {
            all_days.push(existing_data.clone());
        } else {
            all_days.push(DailyRevenueStats {
                date: day.format("%Y-%m-%d").to_string(),
                day_of_week: weekday as i32,
                day_label: day_labels[weekday].to_string(),
                call_count: 0,
                revenue: Decimal::ZERO,
                total_minutes: Decimal::ZERO,
            });
        }
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(RevenueByDayResponse { data: all_days })))
}

/// Get balance trend (last 30 days)
///
/// GET /api/v1/stats/balance-trend
#[instrument(skip(pool, _user))]
pub async fn get_balance_trend(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    debug!("Fetching balance trend");

    let now = Utc::now();

    // Get daily snapshots from balance_transactions or calculate from current state
    // For simplicity, we'll aggregate current balance by date for the last 30 days
    let start_date = (now - Duration::days(30)).date_naive();

    let rows = sqlx::query(
        r#"
        SELECT
            DATE(created_at) as date,
            SUM(new_balance)::NUMERIC as total_balance,
            COUNT(DISTINCT account_id)::BIGINT as account_count
        FROM (
            SELECT DISTINCT ON (account_id, DATE(created_at))
                account_id,
                new_balance,
                created_at
            FROM balance_transactions
            WHERE DATE(created_at) >= $1
            ORDER BY account_id, DATE(created_at), created_at DESC
        ) latest_per_day
        GROUP BY DATE(created_at)
        ORDER BY date
        "#,
    )
    .bind(start_date)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch balance trend: {}", e)))?;

    // If no transaction history, get current balance
    let current_balance_row = sqlx::query(
        r#"
        SELECT
            COALESCE(SUM(balance), 0) as total_balance,
            COUNT(*)::BIGINT as active_accounts
        FROM accounts
        WHERE status = 'active'
        "#,
    )
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch current balance: {}", e)))?;

    let current_total: Decimal = current_balance_row.get("total_balance");
    let current_count: i64 = current_balance_row.get("active_accounts");

    let mut data: Vec<BalanceTrendPoint> = Vec::new();

    // If we have historical data, use it
    if !rows.is_empty() {
        for (index, row) in rows.iter().enumerate() {
            let date: chrono::NaiveDate = row.get("date");
            let total_balance: Decimal = row.get("total_balance");
            let account_count: i64 = row.get("account_count");

            let avg_balance = if account_count > 0 {
                total_balance / Decimal::from(account_count)
            } else {
                Decimal::ZERO
            };

            data.push(BalanceTrendPoint {
                date: date.format("%Y-%m-%d").to_string(),
                day: (index + 1) as i32,
                total_balance,
                active_accounts: account_count,
                average_balance: avg_balance,
            });
        }
    } else {
        // No historical data, create a flat line with current balance
        for i in 0..30 {
            let date = (now - Duration::days(29 - i)).date_naive();
            let avg_balance = if current_count > 0 {
                current_total / Decimal::from(current_count)
            } else {
                Decimal::ZERO
            };

            data.push(BalanceTrendPoint {
                date: date.format("%Y-%m-%d").to_string(),
                day: (i + 1) as i32,
                total_balance: current_total,
                active_accounts: current_count,
                average_balance: avg_balance,
            });
        }
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(BalanceTrendResponse { data })))
}

/// Get calls by type (last 30 days)
///
/// GET /api/v1/stats/calls-by-type
#[instrument(skip(pool, _user))]
pub async fn get_calls_by_type(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    debug!("Fetching calls by type statistics");

    let now = Utc::now();
    let start_time = now - Duration::days(7);

    let rows = sqlx::query(
        r#"
        SELECT
            COALESCE(direction, 'unknown') as call_type,
            COUNT(*)::BIGINT as call_count,
            COALESCE(SUM(duration), 0)::BIGINT as total_duration,
            COALESCE(SUM(cost), 0) as total_cost
        FROM cdrs
        WHERE start_time >= $1
        GROUP BY direction
        ORDER BY call_count DESC
        "#,
    )
    .bind(start_time)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch calls by type: {}", e)))?;

    // Calculate total for percentage
    let total_calls: i64 = rows.iter().map(|r| r.get::<i64, _>("call_count")).sum();

    // Spanish labels for call types
    let type_labels = |call_type: &str| -> String {
        match call_type {
            "outbound" => "Salientes".to_string(),
            "inbound" => "Entrantes".to_string(),
            "internal" => "Internas".to_string(),
            _ => "Desconocido".to_string(),
        }
    };

    let mut data: Vec<CallTypeStats> = Vec::new();
    for row in rows {
        let call_type: String = row.get("call_type");
        let call_count: i64 = row.get("call_count");
        let percentage = if total_calls > 0 {
            (call_count as f64 / total_calls as f64) * 100.0
        } else {
            0.0
        };

        data.push(CallTypeStats {
            call_type: call_type.clone(),
            label: type_labels(&call_type),
            call_count,
            total_duration: row.get("total_duration"),
            total_cost: row.get::<Decimal, _>("total_cost"),
            percentage,
        });
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(CallsByTypeResponse { data })))
}

/// Get calls by zone (top 10 zones, last 30 days)
///
/// GET /api/v1/stats/calls-by-zone
#[instrument(skip(pool, _user))]
pub async fn get_calls_by_zone(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    debug!("Fetching calls by zone statistics");

    let now = Utc::now();
    let start_time = now - Duration::days(30);

    // Use Longest Prefix Match (LPM) to find destination zones
    let rows = sqlx::query(
        r#"
        SELECT
            COALESCE(rc.destination_name, 'Sin Zona') as zone_name,
            COUNT(c.id)::BIGINT as call_count,
            COALESCE(SUM(c.duration), 0)::BIGINT as total_duration,
            COALESCE(SUM(c.cost), 0) as total_cost
        FROM cdrs c
        LEFT JOIN LATERAL (
            SELECT destination_name, destination_prefix
            FROM rate_cards
            WHERE c.called_number LIKE destination_prefix || '%'
            ORDER BY LENGTH(destination_prefix) DESC, priority DESC
            LIMIT 1
        ) rc ON true
        WHERE c.start_time >= $1
        GROUP BY rc.destination_name
        ORDER BY call_count DESC
        LIMIT 10
        "#,
    )
    .bind(start_time)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch calls by zone: {}", e)))?;

    // Calculate total for percentage
    let total_calls: i64 = rows.iter().map(|r| r.get::<i64, _>("call_count")).sum();

    let mut data: Vec<ZoneStats> = Vec::new();
    for row in rows {
        let call_count: i64 = row.get("call_count");
        let percentage = if total_calls > 0 {
            (call_count as f64 / total_calls as f64) * 100.0
        } else {
            0.0
        };

        data.push(ZoneStats {
            zone_id: None,
            zone_name: row.get("zone_name"),
            call_count,
            total_duration: row.get("total_duration"),
            total_cost: row.get::<Decimal, _>("total_cost"),
            percentage,
        });
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(CallsByZoneResponse { data })))
}

/// Get traffic statistics by direction (today)
///
/// GET /api/v1/stats/traffic-by-direction
#[instrument(skip(pool, _user))]
pub async fn get_traffic_by_direction(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    debug!("Fetching traffic statistics by direction");

    let now = Utc::now();
    let start_of_day = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();

    // Get inbound stats
    let inbound_row = sqlx::query(
        r#"
        SELECT
            COUNT(*)::BIGINT as total_calls,
            COALESCE(SUM(billsec) / 60.0, 0) as total_minutes,
            COALESCE(SUM(cost), 0) as total_revenue,
            COALESCE(AVG(duration), 0)::INTEGER as avg_duration
        FROM cdrs
        WHERE start_time >= $1
          AND direction = 'inbound'
        "#,
    )
    .bind(start_of_day)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch inbound stats: {}", e)))?;

    let inbound = TrafficStats {
        direction: "inbound".to_string(),
        label: "Entrantes".to_string(),
        total_calls: inbound_row.get("total_calls"),
        total_minutes: inbound_row.get::<Decimal, _>("total_minutes"),
        total_revenue: inbound_row.get::<Decimal, _>("total_revenue"),
        avg_duration: inbound_row.get("avg_duration"),
    };

    // Get outbound stats
    let outbound_row = sqlx::query(
        r#"
        SELECT
            COUNT(*)::BIGINT as total_calls,
            COALESCE(SUM(billsec) / 60.0, 0) as total_minutes,
            COALESCE(SUM(cost), 0) as total_revenue,
            COALESCE(AVG(duration), 0)::INTEGER as avg_duration
        FROM cdrs
        WHERE start_time >= $1
          AND direction = 'outbound'
        "#,
    )
    .bind(start_of_day)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch outbound stats: {}", e)))?;

    let outbound = TrafficStats {
        direction: "outbound".to_string(),
        label: "Salientes".to_string(),
        total_calls: outbound_row.get("total_calls"),
        total_minutes: outbound_row.get::<Decimal, _>("total_minutes"),
        total_revenue: outbound_row.get::<Decimal, _>("total_revenue"),
        avg_duration: outbound_row.get("avg_duration"),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(TrafficByDirectionResponse {
        inbound,
        outbound,
    })))
}

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
    let network: Vec<String> = NETWORK_FAILURE_CAUSES.iter().map(|c| c.to_string()).collect();
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*)::BIGINT AS attempts,
            COUNT(answer_time)::BIGINT AS answered,
            COALESCE(SUM(billsec) FILTER (WHERE answer_time IS NOT NULL), 0)::BIGINT AS billsec,
            COUNT(*) FILTER (WHERE answer_time IS NULL AND hangup_cause = ANY($3))::BIGINT
                AS network_failures,
            COALESCE(SUM(cost), 0) AS revenue
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
        revenue: row.get::<Decimal, _>("revenue"),
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
        LIMIT 10
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
    })
    .collect();

    Ok(HttpResponse::Ok().json(ApiResponse::success(NocOverviewResponse {
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
    })))
}

/// Configure statistics routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/stats")
            .route("/calls-by-hour", web::get().to(get_calls_by_hour))
            .route("/revenue-by-day", web::get().to(get_revenue_by_day))
            .route("/balance-trend", web::get().to(get_balance_trend))
            .route("/calls-by-type", web::get().to(get_calls_by_type))
            .route("/calls-by-zone", web::get().to(get_calls_by_zone))
            .route("/traffic-by-direction", web::get().to(get_traffic_by_direction))
            .route("/noc", web::get().to(get_noc_overview)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_day_labels() {
        let labels = ["Dom", "Lun", "Mar", "Mié", "Jue", "Vie", "Sáb"];
        assert_eq!(labels.len(), 7);
    }
}
