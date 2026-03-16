//! Active Call handlers
//!
//! HTTP handlers for active call tracking endpoints.

use crate::dto::active_call::{
    ActiveCallRequest, ActiveCallResponse, CdrCreateRequest, CdrCreateResponse,
};
use crate::dto::ApiResponse;
use actix_web::{web, HttpResponse};
use apolo_auth::AuthenticatedUser;
use apolo_core::models::{ActiveCall, Cdr, RateCard};
use apolo_core::traits::{RateRepository, Repository};
use apolo_core::AppError;
use apolo_db::PgRateRepository;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use tracing::{debug, info, instrument, warn};
use validator::Validate;

/// List all active calls
///
/// GET /api/v1/active-calls
#[instrument(skip(pool, _user))]
pub async fn list_active_calls(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    debug!("Listing active calls");

    let rows = sqlx::query(
        r#"
        SELECT
            id, call_id as call_uuid, calling_number, called_number,
            direction, start_time, current_duration, current_cost,
            connection_id as server_id, last_updated as updated_at, server,
            client_id, answer_time, status, rate_per_minute
        FROM active_calls
        ORDER BY start_time DESC
        "#,
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to fetch active calls: {}", e)))?;

    let calls: Vec<ActiveCallResponse> = rows
        .iter()
        .map(|row| {
            // Determine status: if answer_time is set, call is answered; otherwise use stored status or default to ringing
            let answer_time: Option<DateTime<Utc>> = row.get("answer_time");
            let stored_status: Option<String> = row.get("status");
            let status = if answer_time.is_some() {
                "answered".to_string()
            } else {
                stored_status.unwrap_or_else(|| "ringing".to_string())
            };

            ActiveCallResponse {
                call_uuid: row.get("call_uuid"),
                caller_number: row
                    .get::<Option<String>, _>("calling_number")
                    .unwrap_or_default(),
                callee_number: row
                    .get::<Option<String>, _>("called_number")
                    .unwrap_or_default(),
                direction: row
                    .get::<Option<String>, _>("direction")
                    .unwrap_or_else(|| "outbound".to_string()),
                start_time: row.get("start_time"),
                status,
                duration_seconds: row.get::<Option<i32>, _>("current_duration").unwrap_or(0),
                current_cost: row
                    .get::<Option<Decimal>, _>("current_cost")
                    .unwrap_or(Decimal::ZERO),
                zone_name: None,
                rate_per_minute: row.get::<Option<Decimal>, _>("rate_per_minute"),
                account_id: row.get::<Option<i32>, _>("client_id"),
                max_duration: None,
                remaining_duration: None,
                server_id: row
                    .get::<Option<String>, _>("server_id")
                    .or_else(|| row.get("server")),
                updated_at: row.get("updated_at"),
            }
        })
        .collect();

    Ok(HttpResponse::Ok().json(ApiResponse::success(calls)))
}

/// Report/upsert an active call
///
/// POST /api/v1/active-calls
#[instrument(skip(pool, _user, req))]
pub async fn report_active_call(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
    req: web::Json<ActiveCallRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("Active call validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(call_id = %req.call_id, "Reporting active call");

    // Get rate for the called number using LPM
    let rate_repo = PgRateRepository::new(pool.get_ref().clone());
    let rate = if let Some(called) = &req.called_number {
        let normalized = RateCard::normalize_destination(called);
        rate_repo
            .find_by_destination(&normalized)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    // Calculate cost if we have a rate
    let cost = if let Some(ref rate) = rate {
        rate.calculate_cost(req.duration)
    } else {
        req.cost
    };

    let now = Utc::now();
    let start_time = req.start_time.unwrap_or(now);

    // Upsert active call
    let result = sqlx::query(
        r#"
        INSERT INTO active_calls (
            call_id, calling_number, called_number, direction,
            start_time, current_duration, current_cost,
            connection_id, server, last_updated
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ON CONFLICT (call_id) DO UPDATE SET
            current_duration = $6,
            current_cost = $7,
            last_updated = $10
        RETURNING id
        "#,
    )
    .bind(&req.call_id)
    .bind(&req.calling_number)
    .bind(&req.called_number)
    .bind(&req.direction)
    .bind(start_time)
    .bind(req.duration)
    .bind(cost)
    .bind(&req.connection_id)
    .bind(&req.server)
    .bind(now)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to upsert active call: {}", e)))?;

    let id: i32 = result.get("id");

    info!(call_id = %req.call_id, id = id, "Active call reported");

    let response = ActiveCallResponse {
        call_uuid: req.call_id.clone(),
        caller_number: req.calling_number.clone().unwrap_or_default(),
        callee_number: req.called_number.clone().unwrap_or_default(),
        direction: req.direction.clone(),
        start_time,
        status: "dialing".to_string(),
        duration_seconds: req.duration,
        current_cost: cost,
        zone_name: rate.as_ref().map(|r| r.destination_name.clone()),
        rate_per_minute: rate.as_ref().map(|r| r.rate_per_minute),
        account_id: None,
        max_duration: None,
        remaining_duration: None,
        server_id: req.server.clone().or(req.connection_id.clone()),
        updated_at: now,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Remove an active call
///
/// DELETE /api/v1/active-calls/{call_id}
#[instrument(skip(pool, _user))]
pub async fn remove_active_call(
    pool: web::Data<PgPool>,
    path: web::Path<String>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    let call_id = path.into_inner();
    debug!(call_id = %call_id, "Removing active call");

    let result = sqlx::query("DELETE FROM active_calls WHERE call_id = $1")
        .bind(&call_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete active call: {}", e)))?;

    if result.rows_affected() > 0 {
        info!(call_id = %call_id, "Active call removed");
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(AppError::NotFound(format!(
            "Active call {} not found",
            call_id
        )))
    }
}

/// Create a CDR record
///
/// POST /api/v1/cdrs
#[instrument(skip(pool, _user, req))]
pub async fn create_cdr(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
    req: web::Json<CdrCreateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("CDR creation validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(call_uuid = %req.call_uuid, "Creating CDR");

    // Get rate if rate_id provided or lookup by called number
    let rate_repo = PgRateRepository::new(pool.get_ref().clone());
    let rate = if let Some(rate_id) = req.rate_id {
        rate_repo.find_by_id(rate_id).await.ok().flatten()
    } else {
        let normalized = RateCard::normalize_destination(&req.called_number);
        rate_repo
            .find_by_destination(&normalized)
            .await
            .ok()
            .flatten()
    };

    // Calculate cost
    let cost = req
        .cost
        .or_else(|| rate.as_ref().map(|r| r.calculate_cost(req.billsec)));

    let now = Utc::now();

    // Insert CDR
    let result = sqlx::query(
        r#"
        INSERT INTO cdrs (
            call_uuid, account_id, caller_number, called_number,
            start_time, answer_time, end_time, duration, billsec,
            hangup_cause, rate_id, cost, direction, freeswitch_server_id,
            created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        RETURNING id
        "#,
    )
    .bind(&req.call_uuid)
    .bind(req.account_id)
    .bind(&req.caller_number)
    .bind(&req.called_number)
    .bind(req.start_time)
    .bind(req.answer_time)
    .bind(req.end_time)
    .bind(req.duration)
    .bind(req.billsec)
    .bind(&req.hangup_cause)
    .bind(rate.as_ref().map(|r| r.id).or(req.rate_id))
    .bind(cost)
    .bind(&req.direction)
    .bind(&req.freeswitch_server_id)
    .bind(now)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
            AppError::Conflict(format!("CDR for call {} already exists", req.call_uuid))
        } else {
            AppError::Database(format!("Failed to create CDR: {}", e))
        }
    })?;

    let id: i64 = result.get("id");

    info!(
        id = id,
        call_uuid = %req.call_uuid,
        cost = ?cost,
        "CDR created successfully"
    );

    let response = CdrCreateResponse {
        id,
        call_uuid: req.call_uuid.clone(),
        cost,
        message: "CDR created successfully".to_string(),
    };

    Ok(HttpResponse::Created().json(ApiResponse::success(response)))
}

/// Configure active call routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/active-calls")
            .route("", web::get().to(list_active_calls))
            .route("", web::post().to(report_active_call))
            .route("/{call_id}", web::delete().to(remove_active_call)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_active_call_request_validation() {
        let valid_req = ActiveCallRequest {
            call_id: "test-uuid".to_string(),
            calling_number: Some("+51999888777".to_string()),
            called_number: Some("+1555123456".to_string()),
            direction: "outbound".to_string(),
            start_time: None,
            duration: 60,
            cost: Decimal::ZERO,
            connection_id: None,
            server: None,
        };
        assert!(valid_req.validate().is_ok());

        let invalid_req = ActiveCallRequest {
            call_id: "".to_string(),
            ..valid_req
        };
        assert!(invalid_req.validate().is_err());
    }
}
