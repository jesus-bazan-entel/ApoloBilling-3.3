//! Reservation handlers
//!
//! HTTP handlers for balance reservation endpoints.

use actix_web::{web, HttpResponse};
use apolo_core::AppError;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{debug, instrument};
use uuid::Uuid;

/// Reservation response DTO
#[derive(Debug, Serialize)]
pub struct ReservationResponse {
    pub id: String,
    pub account_id: i32,
    pub call_uuid: String,
    pub reserved_amount: f64,
    pub consumed_amount: f64,
    pub released_amount: f64,
    pub status: String,
    pub reservation_type: String,
    pub destination_prefix: Option<String>,
    pub rate_per_minute: f64,
    pub reserved_minutes: i32,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Query parameters for listing reservations
#[derive(Debug, Deserialize)]
pub struct ReservationQueryParams {
    pub status: Option<String>,
    pub account_id: Option<i32>,
}

/// Row struct for database query
#[derive(Debug, sqlx::FromRow)]
struct ReservationRow {
    id: Uuid,
    account_id: i32,
    call_uuid: String,
    reserved_amount: Decimal,
    consumed_amount: Decimal,
    released_amount: Decimal,
    status: String,
    reservation_type: String,
    destination_prefix: Option<String>,
    rate_per_minute: Decimal,
    reserved_minutes: i32,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ReservationRow> for ReservationResponse {
    fn from(row: ReservationRow) -> Self {
        Self {
            id: row.id.to_string(),
            account_id: row.account_id,
            call_uuid: row.call_uuid,
            reserved_amount: row.reserved_amount.to_string().parse().unwrap_or(0.0),
            consumed_amount: row.consumed_amount.to_string().parse().unwrap_or(0.0),
            released_amount: row.released_amount.to_string().parse().unwrap_or(0.0),
            status: row.status,
            reservation_type: row.reservation_type,
            destination_prefix: row.destination_prefix,
            rate_per_minute: row.rate_per_minute.to_string().parse().unwrap_or(0.0),
            reserved_minutes: row.reserved_minutes,
            expires_at: row.expires_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// List reservations with optional filters
///
/// GET /api/v1/reservations
#[instrument(skip(pool))]
pub async fn list_reservations(
    pool: web::Data<PgPool>,
    query: web::Query<ReservationQueryParams>,
) -> Result<HttpResponse, AppError> {
    debug!("Listing reservations with filters: {:?}", query);

    let mut sql = String::from(
        r#"
        SELECT
            id, account_id, call_uuid,
            reserved_amount, consumed_amount, released_amount,
            status, reservation_type, destination_prefix,
            rate_per_minute, reserved_minutes,
            expires_at, created_at, updated_at
        FROM balance_reservations
        WHERE 1=1
        "#,
    );

    if let Some(status) = &query.status {
        sql.push_str(&format!(" AND status = '{}'", status.replace('\'', "''")));
    }

    if let Some(account_id) = query.account_id {
        sql.push_str(&format!(" AND account_id = {}", account_id));
    }

    sql.push_str(" ORDER BY created_at DESC LIMIT 100");

    let rows = sqlx::query_as::<sqlx::Postgres, ReservationRow>(&sql)
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let reservations: Vec<ReservationResponse> = rows.into_iter().map(Into::into).collect();

    Ok(HttpResponse::Ok().json(reservations))
}

/// List active reservations only
///
/// GET /api/v1/reservations/active
#[instrument(skip(pool))]
pub async fn list_active_reservations(pool: web::Data<PgPool>) -> Result<HttpResponse, AppError> {
    debug!("Listing active reservations");

    let rows = sqlx::query_as::<sqlx::Postgres, ReservationRow>(
        r#"
        SELECT
            id, account_id, call_uuid,
            reserved_amount, consumed_amount, released_amount,
            status, reservation_type, destination_prefix,
            rate_per_minute, reserved_minutes,
            expires_at, created_at, updated_at
        FROM balance_reservations
        WHERE status IN ('active', 'partially_consumed')
        AND expires_at > NOW()
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let reservations: Vec<ReservationResponse> = rows.into_iter().map(Into::into).collect();

    Ok(HttpResponse::Ok().json(reservations))
}

/// Configure reservation routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/reservations")
            .route("", web::get().to(list_reservations))
            .route("/active", web::get().to(list_active_reservations)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reservation_response_serialization() {
        let response = ReservationResponse {
            id: "test-uuid".to_string(),
            account_id: 1,
            call_uuid: "call-123".to_string(),
            reserved_amount: 10.0,
            consumed_amount: 5.0,
            released_amount: 0.0,
            status: "active".to_string(),
            reservation_type: "initial".to_string(),
            destination_prefix: Some("51".to_string()),
            rate_per_minute: 0.10,
            reserved_minutes: 5,
            expires_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"account_id\":1"));
        assert!(json.contains("\"status\":\"active\""));
    }
}
