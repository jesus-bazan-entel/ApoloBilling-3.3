//! Legacy Rates handlers
//!
//! HTTP handlers for the legacy /rates API (wrapper around rate-cards).
//! This maintains backwards compatibility with older frontend code.

use crate::dto::rate_card::{RateCardCreateRequest, RateCardResponse};
use crate::dto::{ApiResponse, PaginationParams};
use actix_web::{web, HttpResponse};
use apolo_auth::AuthenticatedUser;
use apolo_core::traits::{RateRepository, Repository};
use apolo_core::AppError;
use apolo_db::PgRateRepository;
use sqlx::PgPool;
use tracing::{debug, info, instrument, warn};
use validator::Validate;

/// List rates with pagination (legacy API)
///
/// GET /api/v1/rates
#[instrument(skip(pool, _user))]
pub async fn list_rates(
    pool: web::Data<PgPool>,
    query: web::Query<PaginationParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    query.validate().map_err(|e| {
        warn!("Pagination validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(
        page = query.page,
        per_page = query.per_page,
        "Listing rates (legacy)"
    );

    let repo = PgRateRepository::new(pool.get_ref().clone());
    let rates = repo.find_all(query.limit(), query.offset()).await?;
    let total = repo.count().await?;

    let response_data: Vec<RateCardResponse> = rates.into_iter().map(|r| r.into()).collect();

    Ok(HttpResponse::Ok().json(query.paginate(response_data, total)))
}

/// Create a rate (legacy API)
///
/// POST /api/v1/rates
#[instrument(skip(pool, _user, req))]
pub async fn create_rate(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
    req: web::Json<RateCardCreateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("Rate creation validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(prefix = %req.destination_prefix, "Creating rate (legacy)");

    let repo = PgRateRepository::new(pool.get_ref().clone());

    // Check for duplicate
    if let Some(_existing) = repo.find_by_destination(&req.destination_prefix).await? {
        return Err(AppError::Conflict(format!(
            "Rate for prefix {} already exists",
            req.destination_prefix
        )));
    }

    let rate_card = req.to_rate_card();
    let created = repo.create(&rate_card).await?;

    info!(id = created.id, prefix = %created.destination_prefix, "Rate created (legacy)");

    let response = RateCardResponse::from(created);
    Ok(HttpResponse::Created().json(ApiResponse::success(response)))
}

/// Delete a rate (legacy API - hard delete)
///
/// DELETE /api/v1/rates/{id}
#[instrument(skip(pool, admin))]
pub async fn delete_rate(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    admin: apolo_auth::AdminUser,
) -> Result<HttpResponse, AppError> {
    let rate_id = path.into_inner();
    debug!(id = rate_id, admin = %admin.username, "Deleting rate (legacy - hard delete)");

    let repo = PgRateRepository::new(pool.get_ref().clone());

    // Verify exists
    let _rate = repo
        .find_by_id(rate_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Rate {} not found", rate_id)))?;

    // Hard delete (unlike rate-cards which does soft delete)
    let deleted = repo.delete(rate_id).await?;

    if deleted {
        info!(id = rate_id, admin = %admin.username, "Rate deleted (legacy)");
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(AppError::Internal("Failed to delete rate".to_string()))
    }
}

/// Configure legacy rates routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/rates")
            .route("", web::get().to(list_rates))
            .route("", web::post().to(create_rate))
            .route("/{id}", web::delete().to(delete_rate)),
    );
}
