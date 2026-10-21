//! Account handlers
//!
//! HTTP handlers for account management endpoints.

use crate::dto::account::{
    AccountCreateRequest, AccountFilterParams, AccountResponse, AccountUpdateRequest,
};
use crate::dto::{ApiResponse, PaginationParams};
use actix_web::{web, HttpResponse};
use apolo_auth::AuthenticatedUser;
use apolo_core::models::{Account, AccountStatus, AuditLogBuilder};
use apolo_core::traits::{AccountRepository, Repository};
use apolo_core::AppError;
use apolo_db::{PgAccountRepository, PgPlanRepository};
use chrono::Utc;
use sqlx::PgPool;
use tracing::{debug, info, instrument, warn};
use validator::Validate;

/// List accounts with pagination and filters
///
/// GET /api/v1/accounts
#[instrument(skip(pool, user))]
pub async fn list_accounts(
    pool: web::Data<PgPool>,
    query: web::Query<PaginationParams>,
    filters: web::Query<AccountFilterParams>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("accounts") {
        return Err(AppError::Forbidden);
    }

    query.validate().map_err(|e| {
        warn!("Pagination validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(
        page = query.page,
        per_page = query.per_page,
        "Listing accounts"
    );

    let repo = PgAccountRepository::new(pool.get_ref().clone());
    let status = filters.status.as_deref();

    let (accounts, total) = repo
        .list_filtered(status, query.limit(), query.offset())
        .await?;

    let response_data: Vec<AccountResponse> = accounts.into_iter().map(|a| a.into()).collect();

    Ok(HttpResponse::Ok().json(query.paginate(response_data, total)))
}

/// Create a new account
///
/// POST /api/v1/accounts
#[instrument(skip(pool, _user, req))]
pub async fn create_account(
    pool: web::Data<PgPool>,
    _user: apolo_auth::AdminUser,
    req: web::Json<AccountCreateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("Account creation validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(account_number = %req.account_number, "Creating account");

    let repo = PgAccountRepository::new(pool.get_ref().clone());

    if let Some(_existing) = repo.find_by_number(&req.account_number).await? {
        warn!(
            account_number = %req.account_number,
            "Account creation failed: duplicate account number"
        );
        return Err(AppError::AlreadyExists(format!(
            "Account {} already exists",
            req.account_number
        )));
    }

    // Resolve bucket configuration: from the plan template when plan_id is
    // set (the plan is the source of truth — any bucket fields on the
    // request are ignored in that case), otherwise directly from the request.
    let (bucket_type, included_minutes, proration_mode, overage_rate, overage_group) =
        if let Some(plan_id) = req.plan_id {
            debug!(plan_id, "Deriving bucket config from plan");

            let plan_repo = PgPlanRepository::new(pool.get_ref().clone());
            let plan = plan_repo.find_by_id(plan_id).await?.ok_or_else(|| {
                warn!(plan_id, "Plan not found");
                AppError::NotFound(format!("Plan {} not found", plan_id))
            })?;

            if req.max_concurrent_calls != plan.max_concurrent_calls {
                return Err(AppError::Validation(format!(
                    "Max concurrent calls must be {} (from plan {})",
                    plan.max_concurrent_calls, plan.plan_code
                )));
            }

            (
                plan.bucket_type,
                plan.included_minutes,
                plan.proration_mode,
                plan.overage_flat_rate_per_minute,
                plan.overage_tariff_group,
            )
        } else {
            let bucket_type = req.bucket_type.clone().ok_or_else(|| {
                AppError::Validation("bucket_type is required when plan_id is not set".to_string())
            })?;
            if bucket_type != "control" && bucket_type != "abierto" {
                return Err(AppError::Validation(
                    "bucket_type must be 'control' or 'abierto'".to_string(),
                ));
            }
            let included_minutes = req.included_minutes.ok_or_else(|| {
                AppError::Validation(
                    "included_minutes is required when plan_id is not set".to_string(),
                )
            })?;

            (
                bucket_type,
                included_minutes,
                req.proration_mode.clone(),
                req.overage_flat_rate_per_minute,
                req.overage_tariff_group.clone(),
            )
        };

    let account = Account {
        account_number: req.account_number.clone(),
        customer_phone: req.customer_phone.clone(),
        max_concurrent_calls: req.max_concurrent_calls,
        plan_id: req.plan_id,
        bucket_type,
        included_minutes,
        proration_mode,
        overage_flat_rate_per_minute: overage_rate,
        overage_tariff_group: overage_group,
        notification_email: req.notification_email.clone(),
        ..Default::default()
    };

    let created = repo.create(&account).await?;

    info!(
        id = created.id,
        account_number = %created.account_number,
        "Account created successfully"
    );

    let audit_details = serde_json::json!({
        "account_number": created.account_number,
        "bucket_type": created.bucket_type,
        "included_minutes": created.included_minutes,
        "plan_id": created.plan_id,
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(_user.username.clone())
        .action("create_account")
        .entity_type("account")
        .entity_id(created.id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    let response = AccountResponse::from(created);
    Ok(HttpResponse::Created().json(ApiResponse::with_message(
        response,
        "Account created successfully",
    )))
}

/// Get a single account by ID
///
/// GET /api/v1/accounts/{id}
#[instrument(skip(pool, user))]
pub async fn get_account(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("accounts") {
        return Err(AppError::Forbidden);
    }

    let account_id = path.into_inner();
    debug!(id = account_id, "Getting account");

    let repo = PgAccountRepository::new(pool.get_ref().clone());
    let account = repo
        .find_by_id(account_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Account {} not found", account_id)))?;

    let response = AccountResponse::from(account);
    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

/// Update an account
///
/// PUT /api/v1/accounts/{id}
#[instrument(skip(pool, _user, req))]
pub async fn update_account(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    _user: apolo_auth::AdminUser,
    req: web::Json<AccountUpdateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("Account update validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    let account_id = path.into_inner();
    debug!(id = account_id, "Updating account");

    let repo = PgAccountRepository::new(pool.get_ref().clone());

    let mut account = repo
        .find_by_id(account_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Account {} not found", account_id)))?;

    let mut changes = serde_json::json!({});

    if let Some(phone) = &req.customer_phone {
        changes["customer_phone"] = serde_json::json!({
            "old": account.customer_phone,
            "new": phone
        });
        account.customer_phone = Some(phone.clone());
    }

    if let Some(status_str) = &req.status {
        if let Some(status) = AccountStatus::from_str(status_str) {
            changes["status"] = serde_json::json!({
                "old": format!("{:?}", account.status),
                "new": format!("{:?}", status)
            });
            account.status = status;
        } else {
            return Err(AppError::Validation(format!(
                "Invalid status: {}",
                status_str
            )));
        }
    }

    if let Some(max_calls) = req.max_concurrent_calls {
        changes["max_concurrent_calls"] = serde_json::json!({
            "old": account.max_concurrent_calls,
            "new": max_calls
        });
        account.max_concurrent_calls = max_calls;
    }

    if let Some(email) = &req.notification_email {
        changes["notification_email"] = serde_json::json!({
            "old": account.notification_email,
            "new": email
        });
        account.notification_email = Some(email.clone());
    }

    account.updated_at = Utc::now();

    let updated = repo.update(&account).await?;

    info!(
        id = updated.id,
        account_number = %updated.account_number,
        "Account updated successfully"
    );

    let audit_details = serde_json::json!({
        "account_number": updated.account_number,
        "changes": changes
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(_user.username.clone())
        .action("update_account")
        .entity_type("account")
        .entity_id(updated.id.to_string())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    let response = AccountResponse::from(updated);
    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        response,
        "Account updated successfully",
    )))
}

/// Delete an account
///
/// DELETE /api/v1/accounts/{id}
#[instrument(skip(pool, admin))]
pub async fn delete_account(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    admin: apolo_auth::AdminUser,
) -> Result<HttpResponse, AppError> {
    let account_id = path.into_inner();
    debug!(
        id = account_id,
        admin = %admin.username,
        "Deleting account"
    );

    let repo = PgAccountRepository::new(pool.get_ref().clone());

    let account = repo
        .find_by_id(account_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Account {} not found", account_id)))?;

    let deleted = repo.delete(account_id).await?;

    if deleted {
        info!(
            id = account_id,
            admin = %admin.username,
            "Account deleted successfully"
        );

        let audit_details = serde_json::json!({
            "account_number": account.account_number,
        });

        if let Ok(audit_data) = AuditLogBuilder::default()
            .username(admin.username.clone())
            .action("delete_account")
            .entity_type("account")
            .entity_id(account_id.to_string())
            .details(audit_details)
            .build()
        {
            audit_data.insert(pool.get_ref()).await;
        }

        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(AppError::Internal("Failed to delete account".to_string()))
    }
}

/// Configure account routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    use super::minute_bucket::{
        get_account_bucket_config, get_minute_bucket_status, minute_bucket_history,
        topup_minute_bucket, update_account_bucket_config, update_minute_bucket_cycle,
    };

    cfg.service(
        web::scope("/accounts")
            .route("", web::get().to(list_accounts))
            .route("", web::post().to(create_account))
            .route("/{id}", web::get().to(get_account))
            .route("/{id}", web::put().to(update_account))
            .route("/{id}", web::delete().to(delete_account))
            // Minute-bucket billing (Plan Control / Plan Abierto) — registered
            // here rather than in minute_bucket::configure() because actix-web
            // commits routing to the first Scope whose prefix matches; a
            // second, separate `web::scope("/accounts")` would 404 on every
            // path below it once this scope claims the "/accounts" prefix.
            .route(
                "/{id}/bucket-config",
                web::get().to(get_account_bucket_config),
            )
            .route(
                "/{id}/bucket-config",
                web::put().to(update_account_bucket_config),
            )
            .route(
                "/{id}/minute-bucket",
                web::get().to(get_minute_bucket_status),
            )
            .route(
                "/{id}/minute-bucket",
                web::put().to(update_minute_bucket_cycle),
            )
            .route(
                "/{id}/minute-bucket/topup",
                web::post().to(topup_minute_bucket),
            )
            .route(
                "/{id}/minute-bucket/history",
                web::get().to(minute_bucket_history),
            ),
    );
}
