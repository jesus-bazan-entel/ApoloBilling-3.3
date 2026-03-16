//! Plan handlers
//!
//! HTTP handlers for plan management endpoints.

use crate::dto::plan::{PlanCreateRequest, PlanResponse, PlanUpdateRequest};
use crate::dto::ApiResponse;
use actix_web::{web, HttpResponse};
use apolo_auth::AuthenticatedUser;
use apolo_core::models::{AccountType, AuditLogBuilder};
use apolo_core::AppError;
use apolo_db::PgPlanRepository;
use sqlx::PgPool;
use tracing::{debug, info, instrument, warn};
use validator::Validate;

/// List all plans
///
/// GET /api/v1/plans
#[instrument(skip(pool, _user))]
pub async fn list_plans(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    debug!("Listing all plans");

    let repo = PgPlanRepository::new(pool.get_ref().clone());
    let plans = repo.list_all().await?;

    let response_data: Vec<PlanResponse> = plans.into_iter().map(Into::into).collect();

    Ok(HttpResponse::Ok().json(ApiResponse::success(response_data)))
}

/// List only active plans
///
/// GET /api/v1/plans/active
#[instrument(skip(pool, _user))]
pub async fn list_active_plans(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    debug!("Listing active plans");

    let repo = PgPlanRepository::new(pool.get_ref().clone());
    let plans = repo.list_active().await?;

    let response_data: Vec<PlanResponse> = plans.into_iter().map(Into::into).collect();

    Ok(HttpResponse::Ok().json(ApiResponse::success(response_data)))
}

/// Get a single plan by ID
///
/// GET /api/v1/plans/{id}
#[instrument(skip(pool, _user))]
pub async fn get_plan(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    let plan_id = path.into_inner();
    debug!(id = plan_id, "Getting plan");

    let repo = PgPlanRepository::new(pool.get_ref().clone());
    let plan = repo
        .find_by_id(plan_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Plan {} not found", plan_id)))?;

    let response = PlanResponse::from(plan);
    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Create a new plan
///
/// POST /api/v1/plans
#[instrument(skip(pool, user, req))]
pub async fn create_plan(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
    req: web::Json<PlanCreateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("Plan creation validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    // Validate business rules
    req.validate_business_rules().map_err(|e| {
        warn!("Plan business validation failed: {}", e);
        AppError::Validation(e)
    })?;

    debug!(plan_code = %req.plan_code, "Creating plan");

    let repo = PgPlanRepository::new(pool.get_ref().clone());

    // Check if plan code already exists
    if let Some(_existing) = repo.find_by_code(&req.plan_code).await? {
        warn!(
            plan_code = %req.plan_code,
            "Plan creation failed: duplicate plan code"
        );
        return Err(AppError::AlreadyExists(format!(
            "Plan with code {} already exists",
            req.plan_code
        )));
    }

    // Parse account type
    let account_type = AccountType::from_str(&req.account_type)
        .ok_or_else(|| AppError::Validation("Invalid account type".to_string()))?;

    // Create plan
    let created = repo
        .create(
            &req.plan_name,
            &req.plan_code,
            &account_type,
            req.initial_balance,
            req.credit_limit,
            req.max_concurrent_calls,
            req.description.as_deref(),
            req.enabled,
            &user.username,
        )
        .await?;

    info!(
        id = created.id,
        plan_code = %created.plan_code,
        "Plan created successfully"
    );

    // Audit log
    let audit_details = serde_json::json!({
        "plan_name": created.plan_name,
        "plan_code": created.plan_code,
        "account_type": format!("{:?}", created.account_type),
        "initial_balance": created.initial_balance,
        "credit_limit": created.credit_limit,
        "max_concurrent_calls": created.max_concurrent_calls,
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(user.username.clone())
        .action("create_plan")
        .entity_type("plan")
        .entity_id(created.id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    let response = PlanResponse::from(created);
    Ok(HttpResponse::Created().json(ApiResponse::with_message(
        response,
        "Plan created successfully",
    )))
}

/// Update an existing plan
///
/// PUT /api/v1/plans/{id}
#[instrument(skip(pool, _user, req))]
pub async fn update_plan(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    _user: AuthenticatedUser,
    req: web::Json<PlanUpdateRequest>,
) -> Result<HttpResponse, AppError> {
    let plan_id = path.into_inner();

    req.validate().map_err(|e| {
        warn!("Plan update validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(id = plan_id, "Updating plan");

    let repo = PgPlanRepository::new(pool.get_ref().clone());

    // Check if plan exists
    let existing = repo
        .find_by_id(plan_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Plan {} not found", plan_id)))?;

    // Build details for audit log
    let mut changes = serde_json::json!({});

    if let Some(name) = &req.plan_name {
        changes["plan_name"] = serde_json::json!({
            "old": existing.plan_name,
            "new": name
        });
    }

    if let Some(balance) = req.initial_balance {
        changes["initial_balance"] = serde_json::json!({
            "old": existing.initial_balance,
            "new": balance
        });
    }

    if let Some(limit) = req.credit_limit {
        changes["credit_limit"] = serde_json::json!({
            "old": existing.credit_limit,
            "new": limit
        });
    }

    if let Some(calls) = req.max_concurrent_calls {
        changes["max_concurrent_calls"] = serde_json::json!({
            "old": existing.max_concurrent_calls,
            "new": calls
        });
    }

    if let Some(desc) = &req.description {
        changes["description"] = serde_json::json!({
            "old": existing.description,
            "new": desc
        });
    }

    if let Some(enabled) = req.enabled {
        changes["enabled"] = serde_json::json!({
            "old": existing.enabled,
            "new": enabled
        });
    }

    // Update plan
    let updated = repo
        .update(
            plan_id,
            req.plan_name.as_deref(),
            req.initial_balance,
            req.credit_limit,
            req.max_concurrent_calls,
            req.description.as_deref(),
            req.enabled,
        )
        .await?;

    info!(id = plan_id, "Plan updated successfully");

    // Audit log with changes
    let audit_details = serde_json::json!({
        "plan_code": updated.plan_code,
        "changes": changes
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(_user.username.clone())
        .action("update_plan")
        .entity_type("plan")
        .entity_id(plan_id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    let response = PlanResponse::from(updated);
    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        response,
        "Plan updated successfully",
    )))
}

/// Delete a plan
///
/// DELETE /api/v1/plans/{id}
#[instrument(skip(pool, _user))]
pub async fn delete_plan(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    let plan_id = path.into_inner();
    debug!(id = plan_id, "Deleting plan");

    let repo = PgPlanRepository::new(pool.get_ref().clone());

    // Check if plan exists
    let existing = repo
        .find_by_id(plan_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Plan {} not found", plan_id)))?;

    // Delete plan
    repo.delete(plan_id).await?;

    info!(id = plan_id, "Plan deleted successfully");

    // Audit log
    let audit_details = serde_json::json!({
        "plan_name": existing.plan_name,
        "plan_code": existing.plan_code,
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(_user.username.clone())
        .action("delete_plan")
        .entity_type("plan")
        .entity_id(plan_id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        (),
        "Plan deleted successfully",
    )))
}

/// Configure plan routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/plans")
            .route("", web::get().to(list_plans))
            .route("", web::post().to(create_plan))
            .route("/active", web::get().to(list_active_plans))
            .route("/{id}", web::get().to(get_plan))
            .route("/{id}", web::put().to(update_plan))
            .route("/{id}", web::delete().to(delete_plan)),
    );
}
