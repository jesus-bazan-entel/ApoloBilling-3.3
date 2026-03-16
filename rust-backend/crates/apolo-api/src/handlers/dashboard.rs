//! Dashboard handlers
//!
//! HTTP handlers for dashboard statistics and metrics.

use actix_web::{web, HttpResponse};
use apolo_core::AppError;
use serde::Serialize;
use sqlx::PgPool;
use tracing::{debug, instrument};

/// Dashboard statistics response
#[derive(Debug, Serialize)]
pub struct DashboardStats {
    /// Total number of accounts
    pub total_accounts: i64,
    /// Number of active accounts
    pub active_accounts: i64,
    /// Total balance across all accounts
    pub total_balance: f64,
    /// Number of active calls currently in progress
    pub active_calls: i64,
    /// Number of active reservations
    pub active_reservations: i64,
    /// Total CDRs today
    pub cdrs_today: i64,
    /// Revenue today
    pub revenue_today: f64,
    /// Total minutes today
    pub minutes_today: f64,
}

/// Get dashboard statistics
///
/// GET /api/v1/stats
#[instrument(skip(pool))]
pub async fn get_stats(pool: web::Data<PgPool>) -> Result<HttpResponse, AppError> {
    debug!("Fetching dashboard statistics");

    // Get account stats
    let account_stats: (i64, i64, Option<rust_decimal::Decimal>) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) as total,
            COUNT(*) FILTER (WHERE status = 'active') as active,
            COALESCE(SUM(balance), 0) as total_balance
        FROM accounts
        "#,
    )
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Get active calls count
    let active_calls: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM active_calls")
        .fetch_one(pool.get_ref())
        .await
        .unwrap_or((0,));

    // Get active reservations count
    let active_reservations: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM balance_reservations
        WHERE status IN ('active', 'partially_consumed')
        AND expires_at > NOW()
        "#,
    )
    .fetch_one(pool.get_ref())
    .await
    .unwrap_or((0,));

    // Get today's CDR stats
    let cdr_stats: (i64, Option<rust_decimal::Decimal>, Option<i64>) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) as total_cdrs,
            COALESCE(SUM(total_cost), 0) as revenue,
            COALESCE(SUM(billsec), 0) as total_seconds
        FROM cdrs
        WHERE start_time >= CURRENT_DATE
        "#,
    )
    .fetch_one(pool.get_ref())
    .await
    .unwrap_or((0, None, None));

    let stats = DashboardStats {
        total_accounts: account_stats.0,
        active_accounts: account_stats.1,
        total_balance: account_stats
            .2
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
            .unwrap_or(0.0),
        active_calls: active_calls.0,
        active_reservations: active_reservations.0,
        cdrs_today: cdr_stats.0,
        revenue_today: cdr_stats
            .1
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
            .unwrap_or(0.0),
        minutes_today: cdr_stats
            .2
            .map(|s| s as f64 / 60.0)
            .unwrap_or(0.0),
    };

    Ok(HttpResponse::Ok().json(stats))
}

/// Configure dashboard routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/stats", web::get().to(get_stats));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_stats_serialization() {
        let stats = DashboardStats {
            total_accounts: 100,
            active_accounts: 85,
            total_balance: 50000.50,
            active_calls: 5,
            active_reservations: 5,
            cdrs_today: 1234,
            revenue_today: 567.89,
            minutes_today: 2500.5,
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"total_accounts\":100"));
        assert!(json.contains("\"active_calls\":5"));
    }
}
