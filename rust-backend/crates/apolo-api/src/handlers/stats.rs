//! Statistics handlers
//!
//! HTTP handlers for dashboard statistics endpoints.

use crate::dto::{
    ApiResponse, BucketTrendPoint, BucketTrendResponse, CallTypeStats, CallsByHourResponse,
    CallsByTypeResponse, CallsByZoneResponse, DailyRevenueStats, HourlyCallStats,
    RevenueByDayResponse, TrafficByDirectionResponse, TrafficStats, ZoneStats,
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

    let now = Utc::now();
    let start_time = now - Duration::hours(24);

    let rows = sqlx::query(
        r#"
        SELECT
            EXTRACT(HOUR FROM start_time)::INTEGER as hour,
            COUNT(*)::BIGINT as call_count,
            COALESCE(SUM(duration), 0)::BIGINT as total_duration,
            COALESCE(SUM(cost), 0) as total_revenue
        FROM cdrs
        WHERE start_time >= $1
        GROUP BY EXTRACT(HOUR FROM start_time)
        ORDER BY hour
        "#,
    )
    .bind(start_time)
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
            hour_map
                .get(&hour)
                .cloned()
                .unwrap_or_else(|| HourlyCallStats {
                    hour,
                    hour_label: format!("{}:00", hour),
                    call_count: 0,
                    total_duration: 0,
                    total_revenue: Decimal::ZERO,
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
        let existing = data
            .iter()
            .find(|d| d.date == day.format("%Y-%m-%d").to_string());

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

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(RevenueByDayResponse {
            data: all_days,
        })),
    )
}

/// Get bucket overage/consumption trend (last 30 days)
///
/// GET /api/v1/stats/bucket-trend
#[instrument(skip(pool, _user))]
pub async fn get_bucket_trend(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    debug!("Fetching bucket overage/consumption trend");

    let now = Utc::now();
    let start_date = (now - Duration::days(30)).date_naive();

    let rows = sqlx::query(
        r#"
        SELECT
            DATE(end_time) as date,
            COALESCE(SUM(overage_cost), 0)::NUMERIC as total_overage_cost,
            COALESCE(SUM(in_bucket_minutes + overage_minutes), 0)::NUMERIC as total_minutes,
            COUNT(DISTINCT account_id)::BIGINT as account_count
        FROM cdrs
        WHERE bucket_cycle_id IS NOT NULL AND DATE(end_time) >= $1
        GROUP BY DATE(end_time)
        ORDER BY date
        "#,
    )
    .bind(start_date)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch bucket trend: {}", e)))?;

    let mut by_date: std::collections::HashMap<chrono::NaiveDate, (Decimal, Decimal, i64)> =
        std::collections::HashMap::new();
    for row in &rows {
        let date: chrono::NaiveDate = row.get("date");
        let cost: Decimal = row.get("total_overage_cost");
        let minutes: Decimal = row.get("total_minutes");
        let accounts: i64 = row.get("account_count");
        by_date.insert(date, (cost, minutes, accounts));
    }

    let mut data: Vec<BucketTrendPoint> = Vec::new();
    for i in 0..30 {
        let date = (now - Duration::days(29 - i)).date_naive();
        let (cost, minutes, accounts) =
            by_date
                .get(&date)
                .cloned()
                .unwrap_or((Decimal::ZERO, Decimal::ZERO, 0));

        data.push(BucketTrendPoint {
            date: date.format("%Y-%m-%d").to_string(),
            day: (i + 1) as i32,
            total_overage_cost: cost,
            total_minutes_consumed: minutes,
            active_accounts: accounts,
        });
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(BucketTrendResponse { data })))
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
    let start_of_day = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();

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

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(TrafficByDirectionResponse {
            inbound,
            outbound,
        })),
    )
}

/// Configure statistics routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/stats")
            .route("/calls-by-hour", web::get().to(get_calls_by_hour))
            .route("/revenue-by-day", web::get().to(get_revenue_by_day))
            .route("/bucket-trend", web::get().to(get_bucket_trend))
            .route("/calls-by-type", web::get().to(get_calls_by_type))
            .route("/calls-by-zone", web::get().to(get_calls_by_zone))
            .route(
                "/traffic-by-direction",
                web::get().to(get_traffic_by_direction),
            ),
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
