//! System settings handlers
//!
//! HTTP handlers for reading and updating system-wide configuration settings.
//! GET is available to all authenticated users; PUT is restricted to superadmins.

use crate::dto::ApiResponse;
use actix_web::{web, HttpResponse};
use apolo_auth::{AuthenticatedUser, SuperadminUser};
use apolo_core::models::AuditLogBuilder;
use apolo_core::AppError;
use sqlx::PgPool;
use tracing::{info, instrument, warn};

/// A single system setting row returned by the API.
#[derive(serde::Serialize, sqlx::FromRow)]
pub struct SystemSetting {
    pub key: String,
    pub value: String,
    pub description: Option<String>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Request body for updating a setting value.
#[derive(serde::Deserialize)]
pub struct UpdateSettingRequest {
    pub value: String,
}

/// Retrieve all system settings.
///
/// GET /api/v1/settings
#[instrument(skip(pool, _user))]
pub async fn get_settings(
    pool: web::Data<PgPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    let settings = sqlx::query_as::<_, SystemSetting>(
        "SELECT key, value, description, updated_at FROM system_settings ORDER BY key",
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        warn!(error = %e, "Failed to query system_settings");
        AppError::Internal(format!("Failed to fetch settings: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(settings)))
}

/// Update a single system setting by key.
///
/// PUT /api/v1/settings/{key}
#[instrument(skip(pool, admin, req))]
pub async fn update_setting(
    pool: web::Data<PgPool>,
    path: web::Path<String>,
    admin: SuperadminUser,
    req: web::Json<UpdateSettingRequest>,
) -> Result<HttpResponse, AppError> {
    let key = path.into_inner();

    if req.value.is_empty() {
        warn!(key = %key, "Setting update rejected: empty value");
        return Err(AppError::Validation(
            "Setting value must not be empty".to_string(),
        ));
    }

    // Verify the key exists before attempting an update.
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM system_settings WHERE key = $1)",
    )
    .bind(&key)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        warn!(error = %e, key = %key, "DB error checking setting existence");
        AppError::Internal(format!("Database error: {}", e))
    })?;

    if !exists {
        return Err(AppError::NotFound(format!("Setting '{}' not found", key)));
    }

    let updated = sqlx::query_as::<_, SystemSetting>(
        r#"
        UPDATE system_settings
        SET value = $1, updated_at = NOW()
        WHERE key = $2
        RETURNING key, value, description, updated_at
        "#,
    )
    .bind(&req.value)
    .bind(&key)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        warn!(error = %e, key = %key, "Failed to update system setting");
        AppError::Internal(format!("Failed to update setting '{}': {}", key, e))
    })?;

    info!(
        key = %updated.key,
        admin = %admin.username,
        "System setting updated"
    );

    let audit_details = serde_json::json!({
        "key": updated.key,
        "new_value": updated.value,
    });

    if let Ok(audit_data) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("update_setting")
        .entity_type("system_setting")
        .entity_id(updated.key.clone())
        .details(audit_details)
        .build()
    {
        audit_data.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        updated,
        "Setting updated successfully",
    )))
}

/// Configure system settings routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/settings")
            .route("", web::get().to(get_settings))
            .route("/{key}", web::put().to(update_setting)),
    );
}
