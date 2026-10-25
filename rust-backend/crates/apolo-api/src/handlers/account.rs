//! Customer (account) handlers
//!
//! HTTP handlers for the customer endpoints. A customer's status gates its SIP
//! devices' REGISTER; there is no balance or rating on this SBC node.

use crate::dto::account::{
    AccountCreateRequest, AccountFilterParams, AccountResponse, AccountUpdateRequest,
};
use crate::dto::{ApiResponse, PaginationParams};
use actix_web::{web, HttpResponse};
use apolo_auth::AuthenticatedUser;
use apolo_core::models::{AccountStatus, AuditLogBuilder};
use apolo_core::traits::{AccountRepository, Repository, SipDeviceRepository};
use apolo_core::AppError;
use apolo_db::{PgAccountRepository, PgSipDeviceRepository};
use chrono::Utc;
use sqlx::PgPool;
use tracing::{debug, info, instrument, warn};
use validator::Validate;

/// List accounts with pagination and filters
///
/// GET /api/v1/accounts
#[instrument(skip(pool, _user))]
pub async fn list_accounts(
    pool: web::Data<PgPool>,
    query: web::Query<PaginationParams>,
    filters: web::Query<AccountFilterParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
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

    // Pass filter parameters as string references
    let status = filters.status.as_deref();
    let search = filters.search.as_deref();

    // Get accounts with filters
    let (accounts, total) = repo
        .list_filtered(status, None, search, query.limit(), query.offset())
        .await?;

    let response_data: Vec<AccountResponse> = accounts.into_iter().map(|a| a.into()).collect();

    Ok(HttpResponse::Ok().json(query.paginate(response_data, total)))
}

/// Global account statistics (totals across all accounts, not page-scoped)
///
/// GET /api/v1/accounts/stats
#[instrument(skip(pool, _user))]
pub async fn account_stats(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    let repo = PgAccountRepository::new(pool.get_ref().clone());
    let stats = repo.stats().await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(stats)))
}

/// Create a new account
///
/// POST /api/v1/accounts
#[instrument(skip(pool, _user, req))]
pub async fn create_account(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
    req: web::Json<AccountCreateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("Account creation validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    debug!(account_number = %req.account_number, "Creating account");

    let repo = PgAccountRepository::new(pool.get_ref().clone());

    // Check if account number already exists
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

    // Create account
    let account = req.to_account();
    let created = repo.create(&account).await?;

    info!(
        id = created.id,
        account_number = %created.account_number,
        "Account created successfully"
    );

    // Audit log
    let audit_details = serde_json::json!({
        "account_number": created.account_number,
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
#[instrument(skip(pool, _user))]
pub async fn get_account(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
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
    _user: AuthenticatedUser,
    req: web::Json<AccountUpdateRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate().map_err(|e| {
        warn!("Account update validation failed: {}", e);
        AppError::Validation(e.to_string())
    })?;

    let account_id = path.into_inner();
    debug!(id = account_id, "Updating account");

    let repo = PgAccountRepository::new(pool.get_ref().clone());

    // Get existing account
    let mut account = repo
        .find_by_id(account_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Account {} not found", account_id)))?;

    // Remember the status before applying updates so we can detect a transition
    // out of "active" (suspended/closed) and force-deregister the account's SIP
    // devices from FreeSWITCH.
    let previous_status = account.status;

    // Build details for audit log
    let mut changes = serde_json::json!({});

    // Apply updates
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

    account.updated_at = Utc::now();

    // Save updates
    let updated = repo.update(&account).await?;

    info!(
        id = updated.id,
        account_number = %updated.account_number,
        "Account updated successfully"
    );

    // If the account just left "active" (suspended or closed), force-deregister
    // its SIP devices so the change takes effect immediately instead of waiting
    // for the current registration to expire. New REGISTERs are already blocked
    // by the directory lookup (INNER JOIN accounts WHERE status = 'active').
    if previous_status.can_make_calls() && !updated.status.can_make_calls() {
        flush_account_registrations(pool.get_ref(), updated.id).await;
    }

    // Audit log with changes
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

/// Force-deregister all SIP devices belonging to an account from FreeSWITCH.
///
/// Called when an account transitions out of `active` (suspended/closed) so the
/// device drops its registration immediately rather than lingering until the
/// current `Expires` window elapses. Uses `fs_cli` (same idiom as the dialplan
/// and routing handlers); failures are logged but never fail the account update,
/// since the directory lookup already blocks any re-REGISTER.
async fn flush_account_registrations(pool: &PgPool, account_id: i32) {
    use std::process::Command;

    let repo = PgSipDeviceRepository::new(pool.clone());
    let devices = match repo.find_by_account(account_id).await {
        Ok(devices) => devices,
        Err(e) => {
            warn!(
                account_id,
                error = %e,
                "Could not load SIP devices to flush registrations after suspension"
            );
            return;
        }
    };

    for device in devices {
        let aor = format!("{}@{}", device.sip_username, device.sip_domain);
        let cmd = format!("sofia profile internal flush_inbound_reg {}", aor);
        match Command::new("fs_cli").args(["-x", &cmd]).output() {
            Ok(output) => {
                info!(
                    account_id,
                    aor = %aor,
                    stdout = %String::from_utf8_lossy(&output.stdout).trim(),
                    "Flushed SIP registration after account suspension"
                );
            }
            Err(e) => {
                warn!(
                    account_id,
                    aor = %aor,
                    error = %e,
                    "Failed to run fs_cli to flush SIP registration"
                );
            }
        }
    }
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

    // Verify account exists
    let account = repo
        .find_by_id(account_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Account {} not found", account_id)))?;

    // Delete account
    let deleted = repo.delete(account_id).await?;

    if deleted {
        info!(
            id = account_id,
            admin = %admin.username,
            "Account deleted successfully"
        );

        // Audit log
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
    cfg.service(
        web::scope("/accounts")
            .route("", web::get().to(list_accounts))
            .route("", web::post().to(create_account))
            .route("/stats", web::get().to(account_stats))
            .route("/{id}", web::get().to(get_account))
            .route("/{id}", web::put().to(update_account))
            .route("/{id}", web::delete().to(delete_account)),
    );
}
