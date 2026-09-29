//! Endpoints handlers
//!
//! HTTP handlers for internal PBX endpoint management (dr_gateways type=9).
//! These represent FreeSWITCH connections synced to Kamailio.

use crate::dto::carriers::{SyncStatus, Transport};
use crate::dto::endpoints::{
    CreateSipEndpointRequest, Endpoint, EndpointListResponse, EndpointSyncResult,
    UpdateSipEndpointRequest,
};
use crate::dto::ApiResponse;
use crate::handlers::kamailio_gateway::{
    gateway_address, gateway_attrs, signalling_from_transport, GW_TYPE_PBX,
};
use actix_web::{web, HttpResponse};
use apolo_auth::{AdminUser, AuthenticatedUser};
use apolo_core::models::AuditLogBuilder;
use apolo_core::AppError;
use sqlx::PgPool;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

// ============================================================================
// Endpoint Handlers
// ============================================================================

/// List all endpoints
///
/// GET /api/v1/endpoints
#[instrument(skip(pool, user))]
pub async fn list_endpoints(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("endpoints") {
        return Err(AppError::Forbidden);
    }

    debug!("Listing all endpoints");

    let endpoints = sqlx::query!(
        r#"
        SELECT id, name, description, host, port, transport,
               fs_profile, kamailio_gwid, sync_status, sync_error,
               enabled, is_default, created_at, updated_at
        FROM endpoints
        ORDER BY name
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let mut enabled_count = 0i64;

    let response: Vec<Endpoint> = endpoints
        .into_iter()
        .map(|r| {
            if r.enabled.unwrap_or(true) {
                enabled_count += 1;
            }

            Endpoint {
                id: r.id,
                name: r.name,
                description: r.description,
                host: r.host,
                port: r.port.unwrap_or(5060),
                transport: r
                    .transport
                    .as_deref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(Transport::Udp),
                fs_profile: r.fs_profile.unwrap_or_else(|| "internal".to_string()),
                kamailio_gwid: r.kamailio_gwid,
                sync_status: r
                    .sync_status
                    .as_deref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(SyncStatus::Pending),
                sync_error: r.sync_error,
                enabled: r.enabled.unwrap_or(true),
                is_default: r.is_default.unwrap_or(false),
                created_at: r.created_at.unwrap_or_else(chrono::Utc::now),
                updated_at: r.updated_at.unwrap_or_else(chrono::Utc::now),
            }
        })
        .collect();

    let total = response.len() as i64;

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(EndpointListResponse {
            endpoints: response,
            total,
            enabled_count,
        })),
    )
}

/// Get a single endpoint by ID
///
/// GET /api/v1/endpoints/{id}
#[instrument(skip(pool, user))]
pub async fn get_endpoint(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("endpoints") {
        return Err(AppError::Forbidden);
    }

    let endpoint_id = path.into_inner();
    debug!(id = %endpoint_id, "Getting endpoint");

    let r = sqlx::query!(
        r#"
        SELECT id, name, description, host, port, transport,
               fs_profile, kamailio_gwid, sync_status, sync_error,
               enabled, is_default, created_at, updated_at
        FROM endpoints
        WHERE id = $1
        "#,
        endpoint_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Endpoint {} not found", endpoint_id)))?;

    let endpoint = Endpoint {
        id: r.id,
        name: r.name,
        description: r.description,
        host: r.host,
        port: r.port.unwrap_or(5060),
        transport: r
            .transport
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(Transport::Udp),
        fs_profile: r.fs_profile.unwrap_or_else(|| "internal".to_string()),
        kamailio_gwid: r.kamailio_gwid,
        sync_status: r
            .sync_status
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(SyncStatus::Pending),
        sync_error: r.sync_error,
        enabled: r.enabled.unwrap_or(true),
        is_default: r.is_default.unwrap_or(false),
        created_at: r.created_at.unwrap_or_else(chrono::Utc::now),
        updated_at: r.updated_at.unwrap_or_else(chrono::Utc::now),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(endpoint)))
}

/// Create a new endpoint
///
/// POST /api/v1/endpoints
#[instrument(skip(pool, admin, req))]
pub async fn create_endpoint(
    pool: web::Data<PgPool>,
    admin: AdminUser,
    req: web::Json<CreateSipEndpointRequest>,
) -> Result<HttpResponse, AppError> {
    info!(name = %req.name, host = %req.host, "Creating endpoint");

    // Validate name
    if req.name.trim().is_empty() {
        return Err(AppError::Validation(
            "Endpoint name cannot be empty".to_string(),
        ));
    }

    // Check for duplicate name
    let existing = sqlx::query!("SELECT id FROM endpoints WHERE name = $1", req.name)
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    if existing.is_some() {
        return Err(AppError::AlreadyExists(format!(
            "Endpoint with name '{}' already exists",
            req.name
        )));
    }

    // If setting as default, clear other defaults
    if req.is_default.unwrap_or(false) {
        sqlx::query!("UPDATE endpoints SET is_default = false WHERE is_default = true")
            .execute(pool.get_ref())
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
    }

    let endpoint_id = Uuid::new_v4();
    let transport = req.transport.clone().unwrap_or(Transport::Udp);
    let port = req.port.unwrap_or(5060);

    sqlx::query!(
        r#"
        INSERT INTO endpoints (
            id, name, description, host, port, transport,
            fs_profile, enabled, is_default, sync_status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'pending')
        "#,
        endpoint_id,
        req.name,
        req.description,
        req.host,
        port,
        transport.to_string(),
        req.fs_profile.as_deref().unwrap_or("internal"),
        req.enabled.unwrap_or(true),
        req.is_default.unwrap_or(false),
    )
    .execute(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Sync to Kamailio
    if let Err(e) = sync_endpoint_to_kamailio(&pool, endpoint_id).await {
        let error_msg = e.to_string();
        let _ = sqlx::query!(
            "UPDATE endpoints SET sync_status = 'error', sync_error = $1 WHERE id = $2",
            &error_msg,
            endpoint_id
        )
        .execute(pool.get_ref())
        .await;
        warn!(error = %error_msg, "Failed to sync endpoint to Kamailio");
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("create_endpoint")
        .entity_type("endpoint")
        .entity_id(endpoint_id.to_string())
        .details(serde_json::json!({
            "name": req.name,
            "host": req.host,
        }))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    get_endpoint(pool, web::Path::from(endpoint_id), admin.0).await
}

/// Update an endpoint
///
/// PUT /api/v1/endpoints/{id}
#[instrument(skip(pool, admin, req))]
pub async fn update_endpoint(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    admin: AdminUser,
    req: web::Json<UpdateSipEndpointRequest>,
) -> Result<HttpResponse, AppError> {
    let endpoint_id = path.into_inner();
    info!(id = %endpoint_id, "Updating endpoint");

    // Check exists
    let _ = sqlx::query!("SELECT id FROM endpoints WHERE id = $1", endpoint_id)
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Endpoint {} not found", endpoint_id)))?;

    // Check for duplicate name if changing
    if let Some(ref name) = req.name {
        let dup = sqlx::query!(
            "SELECT id FROM endpoints WHERE name = $1 AND id != $2",
            name,
            endpoint_id
        )
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        if dup.is_some() {
            return Err(AppError::AlreadyExists(format!(
                "Endpoint with name '{}' already exists",
                name
            )));
        }
    }

    // If setting as default, clear other defaults
    if req.is_default.unwrap_or(false) {
        sqlx::query!(
            "UPDATE endpoints SET is_default = false WHERE is_default = true AND id != $1",
            endpoint_id
        )
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    }

    let transport_str = req.transport.as_ref().map(|t| t.to_string());

    sqlx::query!(
        r#"
        UPDATE endpoints SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            host = COALESCE($4, host),
            port = COALESCE($5, port),
            transport = COALESCE($6, transport),
            fs_profile = COALESCE($7, fs_profile),
            enabled = COALESCE($8, enabled),
            is_default = COALESCE($9, is_default),
            sync_status = 'pending',
            updated_at = NOW()
        WHERE id = $1
        "#,
        endpoint_id,
        req.name,
        req.description,
        req.host,
        req.port,
        transport_str,
        req.fs_profile,
        req.enabled,
        req.is_default,
    )
    .execute(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Sync to Kamailio
    if let Err(e) = sync_endpoint_to_kamailio(&pool, endpoint_id).await {
        let error_msg = e.to_string();
        let _ = sqlx::query!(
            "UPDATE endpoints SET sync_status = 'error', sync_error = $1 WHERE id = $2",
            &error_msg,
            endpoint_id
        )
        .execute(pool.get_ref())
        .await;
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("update_endpoint")
        .entity_type("endpoint")
        .entity_id(endpoint_id.to_string())
        .details(serde_json::json!({"updated": true}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    get_endpoint(pool, web::Path::from(endpoint_id), admin.0).await
}

/// Delete an endpoint
///
/// DELETE /api/v1/endpoints/{id}
#[instrument(skip(pool, admin))]
pub async fn delete_endpoint(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let endpoint_id = path.into_inner();
    info!(id = %endpoint_id, "Deleting endpoint");

    // Get the kamailio_gwid before deletion
    let endpoint = sqlx::query!(
        "SELECT kamailio_gwid FROM endpoints WHERE id = $1",
        endpoint_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Endpoint {} not found", endpoint_id)))?;

    // Delete from PostgreSQL
    let result = sqlx::query!("DELETE FROM endpoints WHERE id = $1", endpoint_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Endpoint {} not found",
            endpoint_id
        )));
    }

    // Delete from Kamailio if synced
    if let Some(gwid) = endpoint.kamailio_gwid {
        if let Err(e) = delete_kamailio_gateway(gwid).await {
            warn!(gwid = gwid, error = %e, "Failed to delete endpoint from Kamailio");
        }
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("delete_endpoint")
        .entity_type("endpoint")
        .entity_id(endpoint_id.to_string())
        .details(serde_json::json!({"deleted": true}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        (),
        "Endpoint deleted successfully",
    )))
}

/// Sync all endpoints to Kamailio
///
/// POST /api/v1/endpoints/sync
#[instrument(skip(pool, _admin))]
pub async fn sync_all_endpoints(
    pool: web::Data<PgPool>,
    _admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    info!("Syncing all endpoints to Kamailio");

    let endpoints = sqlx::query!("SELECT id, name FROM endpoints WHERE enabled = true")
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let mut results = Vec::new();
    let mut succeeded = 0;
    let mut failed = 0;

    for endpoint in endpoints {
        match sync_endpoint_to_kamailio(&pool, endpoint.id).await {
            Ok(gwid) => {
                succeeded += 1;
                results.push(EndpointSyncResult {
                    endpoint_id: endpoint.id,
                    endpoint_name: endpoint.name,
                    kamailio_gwid: gwid,
                    success: true,
                    error: None,
                });
            }
            Err(e) => {
                failed += 1;
                results.push(EndpointSyncResult {
                    endpoint_id: endpoint.id,
                    endpoint_name: endpoint.name,
                    kamailio_gwid: None,
                    success: false,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    // Reload Kamailio routing
    if let Err(e) = reload_kamailio_routing().await {
        warn!(error = %e, "Failed to reload Kamailio routing");
    }

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "total": succeeded + failed,
            "succeeded": succeeded,
            "failed": failed,
            "results": results
        }))),
    )
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Sync a single endpoint to Kamailio dr_gateways (type=9)
async fn sync_endpoint_to_kamailio(
    pool: &PgPool,
    endpoint_id: Uuid,
) -> Result<Option<i32>, AppError> {
    let endpoint = sqlx::query!(
        r#"
        SELECT id, name, host, port, transport, kamailio_gwid, enabled
        FROM endpoints WHERE id = $1
        "#,
        endpoint_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Endpoint {} not found", endpoint_id)))?;

    let address = gateway_address(&endpoint.host, endpoint.port);
    let signalling = signalling_from_transport(endpoint.transport.as_deref());

    // Get Kamailio MySQL connection
    let kamailio_url = std::env::var("KAMAILIO_DATABASE_URL").ok();

    if kamailio_url.is_none() {
        warn!("KAMAILIO_DATABASE_URL not set, skipping Kamailio sync");
        return Ok(None);
    }

    let kamailio_pool = sqlx::mysql::MySqlPool::connect(&kamailio_url.unwrap())
        .await
        .map_err(|e| AppError::Database(format!("Kamailio DB connection failed: {}", e)))?;

    let gwid: i32;

    if let Some(existing_gwid) = endpoint.kamailio_gwid {
        // Update existing gateway - using runtime query for MySQL
        sqlx::query(
            r#"
            UPDATE dr_gateways SET
                address = ?,
                attrs = ?,
                description = ?
            WHERE gwid = ?
            "#,
        )
        .bind(&address)
        .bind(gateway_attrs(existing_gwid, GW_TYPE_PBX, signalling))
        .bind(&endpoint.name)
        .bind(existing_gwid)
        .execute(&kamailio_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to update Kamailio gateway: {}", e)))?;

        gwid = existing_gwid;
    } else {
        // Insert new gateway (type=9 for PBX endpoint) - using runtime query for MySQL
        let result = sqlx::query(
            r#"
            INSERT INTO dr_gateways (type, address, strip, attrs, description)
            VALUES (?, ?, 0, '', ?)
            "#,
        )
        .bind(GW_TYPE_PBX)
        .bind(&address)
        .bind(&endpoint.name)
        .execute(&kamailio_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to insert Kamailio gateway: {}", e)))?;

        gwid = result.last_insert_id() as i32;

        // attrs starts with the gwid, which is only known once the INSERT has assigned it
        sqlx::query("UPDATE dr_gateways SET attrs = ? WHERE gwid = ?")
            .bind(gateway_attrs(gwid, GW_TYPE_PBX, signalling))
            .bind(gwid)
            .execute(&kamailio_pool)
            .await
            .map_err(|e| {
                AppError::Database(format!("Failed to set Kamailio gateway attrs: {}", e))
            })?;

        // Update PostgreSQL with the new gwid
        sqlx::query!(
            "UPDATE endpoints SET kamailio_gwid = $1, sync_status = 'synced', sync_error = NULL WHERE id = $2",
            gwid,
            endpoint_id
        )
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    }

    // Mark as synced
    sqlx::query!(
        "UPDATE endpoints SET sync_status = 'synced', sync_error = NULL WHERE id = $1",
        endpoint_id
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(Some(gwid))
}

/// Delete a gateway from Kamailio
async fn delete_kamailio_gateway(gwid: i32) -> Result<(), AppError> {
    let kamailio_url = std::env::var("KAMAILIO_DATABASE_URL").ok();

    if kamailio_url.is_none() {
        return Ok(());
    }

    let kamailio_pool = sqlx::mysql::MySqlPool::connect(&kamailio_url.unwrap())
        .await
        .map_err(|e| AppError::Database(format!("Kamailio DB connection failed: {}", e)))?;

    sqlx::query("DELETE FROM dr_gateways WHERE gwid = ?")
        .bind(gwid)
        .execute(&kamailio_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete Kamailio gateway: {}", e)))?;

    Ok(())
}

/// Reload Kamailio routing tables
async fn reload_kamailio_routing() -> Result<(), AppError> {
    use tokio::process::Command;

    let output = Command::new("kamcmd")
        .args(["drouting.reload"])
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to execute kamcmd: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!(error = %stderr, "Failed to reload Kamailio drouting");
    }

    Ok(())
}

// ============================================================================
// Route Configuration
// ============================================================================

/// Configure endpoints routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/endpoints")
            .route("", web::get().to(list_endpoints))
            .route("", web::post().to(create_endpoint))
            .route("/sync", web::post().to(sync_all_endpoints))
            .route("/{id}", web::get().to(get_endpoint))
            .route("/{id}", web::put().to(update_endpoint))
            .route("/{id}", web::delete().to(delete_endpoint)),
    );
}
