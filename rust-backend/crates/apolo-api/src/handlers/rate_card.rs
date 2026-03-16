//! Rate Card handlers
//!
//! HTTP handlers for rate card management endpoints.

use crate::dto::rate_card::{
    BulkCreateRequest, BulkCreateResponse, BulkError, RateCardCreateRequest, RateCardFilterParams,
    RateCardResponse, RateCardUpdateRequest, RateSearchResponse,
};
use crate::dto::{ApiResponse, PaginationParams};
use actix_web::{web, HttpResponse};
use apolo_auth::AuthenticatedUser;
use apolo_core::models::{AuditLogBuilder, RateCard};
use apolo_core::traits::{RateRepository, Repository};
use apolo_core::AppError;
use apolo_db::PgRateRepository;
use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use tracing::{debug, info, instrument, warn};
use validator::Validate;

/// List rate cards with pagination and filters
///
/// GET /api/v1/rate-cards
#[instrument(skip(pool, _user))]
pub async fn list_rate_cards(
    pool: web::Data<PgPool>,
    query: web::Query<PaginationParams>,
    filters: web::Query<RateCardFilterParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    query.validate().map_err(|e| {
        warn!("Pagination validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(
        page = query.page,
        per_page = query.per_page,
        prefix = ?filters.prefix,
        name = ?filters.name,
        "Listing rate cards"
    );

    let repo = PgRateRepository::new(pool.get_ref().clone());

    // Get rate cards with optional search
    let (rates, total) = if filters.prefix.is_some() || filters.name.is_some() {
        let prefix = filters.prefix.as_deref();
        let name = filters.name.as_deref();
        repo.search(prefix, name, query.limit(), query.offset())
            .await?
    } else {
        let rates = repo.find_all(query.limit(), query.offset()).await?;
        let total = repo.count().await?;
        (rates, total)
    };

    // Filter effective only if requested
    let rates = if filters.effective_only {
        rates.into_iter().filter(|r| r.is_effective()).collect()
    } else {
        rates
    };

    let response_data: Vec<RateCardResponse> = rates.into_iter().map(|r| r.into()).collect();

    Ok(HttpResponse::Ok().json(query.paginate(response_data, total)))
}

/// Create a new rate card
///
/// POST /api/v1/rate-cards
#[instrument(skip(pool, user, req))]
pub async fn create_rate_card(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
    req: web::Json<RateCardCreateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("Rate card creation validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(
        prefix = %req.destination_prefix,
        name = %req.destination_name,
        "Creating rate card"
    );

    let repo = PgRateRepository::new(pool.get_ref().clone());

    // Check for duplicate prefix
    if let Some(_existing) = repo.find_by_destination(&req.destination_prefix).await? {
        warn!(prefix = %req.destination_prefix, "Rate card creation failed: duplicate prefix");
        return Err(AppError::Conflict(format!(
            "Rate card for prefix {} already exists",
            req.destination_prefix
        )));
    }

    // Create rate card
    let rate_card = req.to_rate_card();
    let created = repo.create(&rate_card).await?;

    info!(
        id = created.id,
        prefix = %created.destination_prefix,
        "Rate card created successfully"
    );

    // Audit log
    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(user.username.clone())
        .action("create_rate_card")
        .entity_type("rate_card")
        .entity_id(created.id.to_string())
        .details(json!({
            "destination_prefix": created.destination_prefix,
            "destination_name": created.destination_name,
            "rate_per_minute": created.rate_per_minute,
            "billing_increment": created.billing_increment
        }))
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    let response = RateCardResponse::from(created);
    Ok(HttpResponse::Created().json(ApiResponse::with_message(
        response,
        "Rate card created successfully",
    )))
}

/// Get a single rate card by ID
///
/// GET /api/v1/rate-cards/{id}
#[instrument(skip(pool, _user))]
pub async fn get_rate_card(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    let rate_id = path.into_inner();
    debug!(id = rate_id, "Getting rate card");

    let repo = PgRateRepository::new(pool.get_ref().clone());
    let rate = repo
        .find_by_id(rate_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Rate card {} not found", rate_id)))?;

    let response = RateCardResponse::from(rate);
    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Update a rate card
///
/// PUT /api/v1/rate-cards/{id}
#[instrument(skip(pool, user, req))]
pub async fn update_rate_card(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    user: AuthenticatedUser,
    req: web::Json<RateCardUpdateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("Rate card update validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    let rate_id = path.into_inner();
    debug!(id = rate_id, "Updating rate card");

    let repo = PgRateRepository::new(pool.get_ref().clone());

    // Get existing rate card
    let mut rate = repo
        .find_by_id(rate_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Rate card {} not found", rate_id)))?;

    // Build details for audit log
    let mut changes = json!({});

    // Apply updates
    if let Some(name) = &req.destination_name {
        changes["destination_name"] = json!({ "old": rate.destination_name, "new": name });
        rate.destination_name = name.clone();
        rate.rate_name = Some(name.clone());
    }

    if let Some(rate_pm) = req.rate_per_minute {
        changes["rate_per_minute"] = json!({ "old": rate.rate_per_minute, "new": rate_pm });
        rate.rate_per_minute = rate_pm;
    }

    if let Some(increment) = req.billing_increment {
        changes["billing_increment"] = json!({ "old": rate.billing_increment, "new": increment });
        rate.billing_increment = increment;
    }

    if let Some(fee) = req.connection_fee {
        changes["connection_fee"] = json!({ "old": rate.connection_fee, "new": fee });
        rate.connection_fee = fee;
    }

    if let Some(priority) = req.priority {
        changes["priority"] = json!({ "old": rate.priority, "new": priority });
        rate.priority = priority;
    }

    if let Some(end) = req.effective_end {
        changes["effective_end"] = json!({ "old": rate.effective_end, "new": end });
        rate.effective_end = Some(end);
    }

    rate.updated_at = Utc::now();

    // Save updates
    let updated = repo.update(&rate).await?;

    info!(
        id = updated.id,
        prefix = %updated.destination_prefix,
        "Rate card updated successfully"
    );

    // Audit log
    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(user.username.clone())
        .action("update_rate_card")
        .entity_type("rate_card")
        .entity_id(updated.id.to_string())
        .details(json!({
            "destination_prefix": updated.destination_prefix,
            "changes": changes
        }))
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    let response = RateCardResponse::from(updated);
    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        response,
        "Rate card updated successfully",
    )))
}

/// Soft delete a rate card (sets effective_end to now)
///
/// DELETE /api/v1/rate-cards/{id}
#[instrument(skip(pool, user))]
pub async fn delete_rate_card(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    let rate_id = path.into_inner();
    debug!(id = rate_id, "Soft deleting rate card");

    let repo = PgRateRepository::new(pool.get_ref().clone());

    // Get existing rate card
    let mut rate = repo
        .find_by_id(rate_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Rate card {} not found", rate_id)))?;

    // Store info for audit log before deletion
    let prefix = rate.destination_prefix.clone();
    let name = rate.destination_name.clone();

    // Soft delete by setting effective_end
    rate.effective_end = Some(Utc::now());
    rate.updated_at = Utc::now();

    repo.update(&rate).await?;

    info!(id = rate_id, "Rate card soft deleted successfully");

    // Audit log
    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(user.username.clone())
        .action("delete_rate_card")
        .entity_type("rate_card")
        .entity_id(rate_id.to_string())
        .details(json!({
            "destination_prefix": prefix,
            "destination_name": name,
            "deleted_at": Utc::now()
        }))
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::NoContent().finish())
}

/// Bulk create rate cards
///
/// POST /api/v1/rate-cards/bulk
#[instrument(skip(pool, admin, req))]
pub async fn bulk_create_rate_cards(
    pool: web::Data<PgPool>,
    admin: apolo_auth::AdminUser,
    req: web::Json<BulkCreateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("Bulk create validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(
        count = req.rates.len(),
        admin = %admin.username,
        "Bulk creating rate cards"
    );

    let repo = PgRateRepository::new(pool.get_ref().clone());

    let mut created_count = 0;
    let mut error_count = 0;
    let mut created_prefixes = Vec::new();
    let mut errors_detail = Vec::new();

    // Process each rate card
    for rate_req in &req.rates {
        if let Err(e) = rate_req.validate() {
            error_count += 1;
            errors_detail.push(BulkError {
                prefix: rate_req.destination_prefix.clone(),
                error: e.to_string(),
            });
            continue;
        }

        // Check for duplicate
        if let Ok(Some(_)) = repo.find_by_destination(&rate_req.destination_prefix).await {
            error_count += 1;
            errors_detail.push(BulkError {
                prefix: rate_req.destination_prefix.clone(),
                error: "Prefix already exists".to_string(),
            });
            continue;
        }

        // Create rate card
        let rate_card = rate_req.to_rate_card();
        match repo.create(&rate_card).await {
            Ok(created) => {
                created_count += 1;
                created_prefixes.push(created.destination_prefix);
            }
            Err(e) => {
                error_count += 1;
                errors_detail.push(BulkError {
                    prefix: rate_req.destination_prefix.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    info!(
        created = created_count,
        errors = error_count,
        admin = %admin.username,
        "Bulk create completed"
    );

    let response = BulkCreateResponse {
        created: created_count,
        errors: error_count,
        created_prefixes,
        errors_detail,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Search rate for a phone number using LPM
///
/// GET /api/v1/rate-cards/search/{phone_number}
#[instrument(skip(pool, _user))]
pub async fn search_rate_for_number(
    pool: web::Data<PgPool>,
    path: web::Path<String>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    let phone_number = path.into_inner();
    debug!(phone = %phone_number, "Searching rate for phone number");

    // Normalize phone number
    let normalized = RateCard::normalize_destination(&phone_number);

    if normalized.is_empty() {
        return Err(AppError::Validation(
            "Invalid phone number format".to_string(),
        ));
    }

    let repo = PgRateRepository::new(pool.get_ref().clone());

    // Find rate using LPM
    let rate = repo
        .find_by_destination(&normalized)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("No rate found for number {}", phone_number)))?;

    let response = RateSearchResponse::from_rate_card(&rate, &phone_number);
    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Configure rate card routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/rate-cards")
            .route("", web::get().to(list_rate_cards))
            .route("", web::post().to(create_rate_card))
            .route("/bulk", web::post().to(bulk_create_rate_cards))
            .route(
                "/search/{phone_number}",
                web::get().to(search_rate_for_number),
            )
            .route("/{id}", web::get().to(get_rate_card))
            .route("/{id}", web::put().to(update_rate_card))
            .route("/{id}", web::delete().to(delete_rate_card)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_rate_card_create_validation() {
        let valid_req = RateCardCreateRequest {
            destination_prefix: "51".to_string(),
            destination_name: "Peru".to_string(),
            rate_per_minute: dec!(0.05),
            billing_increment: 6,
            connection_fee: dec!(0),
            priority: 0,
        };
        assert!(valid_req.validate().is_ok());

        let invalid_req = RateCardCreateRequest {
            destination_prefix: "".to_string(),
            destination_name: "".to_string(),
            rate_per_minute: dec!(0.05),
            billing_increment: 6,
            connection_fee: dec!(0),
            priority: 0,
        };
        assert!(invalid_req.validate().is_err());
    }
}
