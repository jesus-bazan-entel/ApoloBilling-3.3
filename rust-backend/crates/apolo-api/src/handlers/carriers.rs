//! Carriers handlers
//!
//! HTTP handlers for external SIP carrier management (dr_gateways type=8).
//! Syncs to Kamailio MySQL database.

use crate::dto::carriers::{
    BulkSyncResult, Carrier, CarrierListResponse, CarrierSipCheckResult, CarrierSyncResult,
    CreateCarrierRequest, SipStatus, SyncStatus, Transport, UpdateCarrierRequest,
};
use crate::dto::ApiResponse;
use crate::handlers::kamailio_gateway::{
    gateway_address, gateway_attrs, signalling_from_transport, GW_TYPE_CARRIER,
};
use actix_web::{web, HttpResponse};
use apolo_auth::{AdminUser, AuthenticatedUser};
use apolo_core::models::AuditLogBuilder;
use apolo_core::AppError;
use sqlx::PgPool;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

// ============================================================================
// Carrier Handlers
// ============================================================================

/// List all carriers
///
/// GET /api/v1/carriers
#[instrument(skip(pool, user))]
pub async fn list_carriers(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("carriers") {
        return Err(AppError::Forbidden);
    }

    debug!("Listing all carriers");

    let carriers = sqlx::query!(
        r#"
        SELECT id, name, description, host, port, transport,
               auth_username, auth_password, strip_digits, prefix_to_add,
               kamailio_gwid, sync_status, sync_error,
               sip_status, last_options_check, last_options_latency_ms,
               enabled, created_at, updated_at
        FROM carriers
        ORDER BY name
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let mut online_count = 0i64;
    let mut offline_count = 0i64;

    let response: Vec<Carrier> = carriers
        .into_iter()
        .map(|r| {
            let sip_status = r
                .sip_status
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(SipStatus::Unknown);

            match sip_status {
                SipStatus::Reachable => online_count += 1,
                SipStatus::Unreachable => offline_count += 1,
                _ => {}
            }

            Carrier {
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
                auth_username: r.auth_username,
                auth_password: None, // Never expose password
                strip_digits: r.strip_digits.unwrap_or(0),
                prefix_to_add: r.prefix_to_add.unwrap_or_default(),
                kamailio_gwid: r.kamailio_gwid,
                sync_status: r
                    .sync_status
                    .as_deref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(SyncStatus::Pending),
                sync_error: r.sync_error,
                sip_status,
                last_options_check: r.last_options_check,
                last_options_latency_ms: r.last_options_latency_ms,
                enabled: r.enabled.unwrap_or(true),
                created_at: r.created_at.unwrap_or_else(chrono::Utc::now),
                updated_at: r.updated_at.unwrap_or_else(chrono::Utc::now),
            }
        })
        .collect();

    let total = response.len() as i64;

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(CarrierListResponse {
            carriers: response,
            total,
            online_count,
            offline_count,
        })),
    )
}

/// Get a single carrier by ID
///
/// GET /api/v1/carriers/{id}
#[instrument(skip(pool, user))]
pub async fn get_carrier(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("carriers") {
        return Err(AppError::Forbidden);
    }

    let carrier_id = path.into_inner();
    debug!(id = %carrier_id, "Getting carrier");

    let r = sqlx::query!(
        r#"
        SELECT id, name, description, host, port, transport,
               auth_username, auth_password, strip_digits, prefix_to_add,
               kamailio_gwid, sync_status, sync_error,
               sip_status, last_options_check, last_options_latency_ms,
               enabled, created_at, updated_at
        FROM carriers
        WHERE id = $1
        "#,
        carrier_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Carrier {} not found", carrier_id)))?;

    let carrier = Carrier {
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
        auth_username: r.auth_username,
        auth_password: None,
        strip_digits: r.strip_digits.unwrap_or(0),
        prefix_to_add: r.prefix_to_add.unwrap_or_default(),
        kamailio_gwid: r.kamailio_gwid,
        sync_status: r
            .sync_status
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(SyncStatus::Pending),
        sync_error: r.sync_error,
        sip_status: r
            .sip_status
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(SipStatus::Unknown),
        last_options_check: r.last_options_check,
        last_options_latency_ms: r.last_options_latency_ms,
        enabled: r.enabled.unwrap_or(true),
        created_at: r.created_at.unwrap_or_else(chrono::Utc::now),
        updated_at: r.updated_at.unwrap_or_else(chrono::Utc::now),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(carrier)))
}

/// Create a new carrier
///
/// POST /api/v1/carriers
#[instrument(skip(pool, admin, req))]
pub async fn create_carrier(
    pool: web::Data<PgPool>,
    admin: AdminUser,
    req: web::Json<CreateCarrierRequest>,
) -> Result<HttpResponse, AppError> {
    info!(name = %req.name, host = %req.host, "Creating carrier");

    // Validate name
    if req.name.trim().is_empty() {
        return Err(AppError::Validation(
            "Carrier name cannot be empty".to_string(),
        ));
    }

    // Check for duplicate name
    let existing = sqlx::query!("SELECT id FROM carriers WHERE name = $1", req.name)
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    if existing.is_some() {
        return Err(AppError::AlreadyExists(format!(
            "Carrier with name '{}' already exists",
            req.name
        )));
    }

    let carrier_id = Uuid::new_v4();
    let transport = req.transport.clone().unwrap_or(Transport::Udp);
    let port = req.port.unwrap_or(5060);

    sqlx::query!(
        r#"
        INSERT INTO carriers (
            id, name, description, host, port, transport,
            auth_username, auth_password, strip_digits, prefix_to_add,
            enabled, sync_status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'pending')
        "#,
        carrier_id,
        req.name,
        req.description,
        req.host,
        port,
        transport.to_string(),
        req.auth_username,
        req.auth_password,
        req.strip_digits.unwrap_or(0),
        req.prefix_to_add.as_deref().unwrap_or(""),
        req.enabled.unwrap_or(true),
    )
    .execute(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Sync to Kamailio
    if let Err(e) = sync_carrier_to_kamailio(&pool, carrier_id).await {
        let error_msg = e.to_string();
        let _ = sqlx::query!(
            "UPDATE carriers SET sync_status = 'error', sync_error = $1 WHERE id = $2",
            &error_msg,
            carrier_id
        )
        .execute(pool.get_ref())
        .await;
        warn!(error = %error_msg, "Failed to sync carrier to Kamailio");
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("create_carrier")
        .entity_type("carrier")
        .entity_id(carrier_id.to_string())
        .details(serde_json::json!({
            "name": req.name,
            "host": req.host,
        }))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    get_carrier(pool, web::Path::from(carrier_id), admin.0).await
}

/// Update a carrier
///
/// PUT /api/v1/carriers/{id}
#[instrument(skip(pool, admin, req))]
pub async fn update_carrier(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    admin: AdminUser,
    req: web::Json<UpdateCarrierRequest>,
) -> Result<HttpResponse, AppError> {
    let carrier_id = path.into_inner();
    info!(id = %carrier_id, "Updating carrier");

    // Check exists
    let _ = sqlx::query!("SELECT id FROM carriers WHERE id = $1", carrier_id)
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Carrier {} not found", carrier_id)))?;

    // Check for duplicate name if changing
    if let Some(ref name) = req.name {
        let dup = sqlx::query!(
            "SELECT id FROM carriers WHERE name = $1 AND id != $2",
            name,
            carrier_id
        )
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        if dup.is_some() {
            return Err(AppError::AlreadyExists(format!(
                "Carrier with name '{}' already exists",
                name
            )));
        }
    }

    let transport_str = req.transport.as_ref().map(|t| t.to_string());

    sqlx::query!(
        r#"
        UPDATE carriers SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            host = COALESCE($4, host),
            port = COALESCE($5, port),
            transport = COALESCE($6, transport),
            auth_username = COALESCE($7, auth_username),
            auth_password = COALESCE($8, auth_password),
            strip_digits = COALESCE($9, strip_digits),
            prefix_to_add = COALESCE($10, prefix_to_add),
            enabled = COALESCE($11, enabled),
            sync_status = 'pending',
            updated_at = NOW()
        WHERE id = $1
        "#,
        carrier_id,
        req.name,
        req.description,
        req.host,
        req.port,
        transport_str,
        req.auth_username,
        req.auth_password,
        req.strip_digits,
        req.prefix_to_add,
        req.enabled,
    )
    .execute(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Sync to Kamailio
    if let Err(e) = sync_carrier_to_kamailio(&pool, carrier_id).await {
        let error_msg = e.to_string();
        let _ = sqlx::query!(
            "UPDATE carriers SET sync_status = 'error', sync_error = $1 WHERE id = $2",
            &error_msg,
            carrier_id
        )
        .execute(pool.get_ref())
        .await;
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("update_carrier")
        .entity_type("carrier")
        .entity_id(carrier_id.to_string())
        .details(serde_json::json!({"updated": true}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    get_carrier(pool, web::Path::from(carrier_id), admin.0).await
}

/// Delete a carrier
///
/// DELETE /api/v1/carriers/{id}
#[instrument(skip(pool, admin))]
pub async fn delete_carrier(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let carrier_id = path.into_inner();
    info!(id = %carrier_id, "Deleting carrier");

    // Get the kamailio_gwid before deletion
    let carrier = sqlx::query!(
        "SELECT kamailio_gwid FROM carriers WHERE id = $1",
        carrier_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Carrier {} not found", carrier_id)))?;

    // Delete from PostgreSQL
    let result = sqlx::query!("DELETE FROM carriers WHERE id = $1", carrier_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Carrier {} not found",
            carrier_id
        )));
    }

    // Delete from Kamailio if synced
    if let Some(gwid) = carrier.kamailio_gwid {
        if let Err(e) = delete_kamailio_gateway(gwid).await {
            warn!(gwid = gwid, error = %e, "Failed to delete carrier from Kamailio");
        }
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("delete_carrier")
        .entity_type("carrier")
        .entity_id(carrier_id.to_string())
        .details(serde_json::json!({"deleted": true}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        (),
        "Carrier deleted successfully",
    )))
}

/// Check SIP OPTIONS status for a carrier
///
/// POST /api/v1/carriers/{id}/check-status
#[instrument(skip(pool, _admin))]
pub async fn check_carrier_status(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    _admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let carrier_id = path.into_inner();
    debug!(id = %carrier_id, "Checking carrier SIP status");

    let carrier = sqlx::query!(
        "SELECT id, name, host, port FROM carriers WHERE id = $1",
        carrier_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Carrier {} not found", carrier_id)))?;

    // Execute SIP OPTIONS check via sipsak or similar
    let check_result = check_sip_options(&carrier.host, carrier.port.unwrap_or(5060)).await;

    // Update carrier status
    sqlx::query!(
        r#"
        UPDATE carriers SET
            sip_status = $2,
            last_options_check = NOW(),
            last_options_latency_ms = $3
        WHERE id = $1
        "#,
        carrier_id,
        check_result.status.to_string(),
        check_result.latency_ms,
    )
    .execute(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let result = CarrierSipCheckResult {
        carrier_id,
        carrier_name: carrier.name,
        status: check_result.status,
        latency_ms: check_result.latency_ms,
        response_code: check_result.response_code,
        error_message: check_result.error,
        checked_at: chrono::Utc::now(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

/// Sync all carriers to Kamailio
///
/// POST /api/v1/carriers/sync
#[instrument(skip(pool, _admin))]
pub async fn sync_all_carriers(
    pool: web::Data<PgPool>,
    _admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    info!("Syncing all carriers to Kamailio");

    let carriers = sqlx::query!("SELECT id, name FROM carriers WHERE enabled = true")
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let mut results = Vec::new();
    let mut succeeded = 0;
    let mut failed = 0;

    for carrier in carriers {
        match sync_carrier_to_kamailio(&pool, carrier.id).await {
            Ok(gwid) => {
                succeeded += 1;
                results.push(CarrierSyncResult {
                    carrier_id: carrier.id,
                    carrier_name: carrier.name,
                    kamailio_gwid: gwid,
                    success: true,
                    error: None,
                });
            }
            Err(e) => {
                failed += 1;
                results.push(CarrierSyncResult {
                    carrier_id: carrier.id,
                    carrier_name: carrier.name,
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

    let total = succeeded + failed;
    Ok(
        HttpResponse::Ok().json(ApiResponse::success(BulkSyncResult {
            total,
            succeeded,
            failed,
            results,
        })),
    )
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Sync a single carrier to Kamailio dr_gateways
async fn sync_carrier_to_kamailio(
    pool: &PgPool,
    carrier_id: Uuid,
) -> Result<Option<i32>, AppError> {
    let carrier = sqlx::query!(
        r#"
        SELECT id, name, host, port, transport, strip_digits, kamailio_gwid
        FROM carriers WHERE id = $1
        "#,
        carrier_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Carrier {} not found", carrier_id)))?;

    let address = gateway_address(&carrier.host, carrier.port);
    let signalling = signalling_from_transport(carrier.transport.as_deref());

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

    if let Some(existing_gwid) = carrier.kamailio_gwid {
        // Update existing gateway - using runtime query for MySQL
        sqlx::query(
            r#"
            UPDATE dr_gateways SET
                address = ?,
                strip = ?,
                attrs = ?,
                description = ?
            WHERE gwid = ?
            "#,
        )
        .bind(&address)
        .bind(carrier.strip_digits.unwrap_or(0))
        .bind(gateway_attrs(existing_gwid, GW_TYPE_CARRIER, signalling))
        .bind(&carrier.name)
        .bind(existing_gwid)
        .execute(&kamailio_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to update Kamailio gateway: {}", e)))?;

        gwid = existing_gwid;
    } else {
        // Insert new gateway (type=8 for carrier) - using runtime query for MySQL
        let result = sqlx::query(
            r#"
            INSERT INTO dr_gateways (type, address, strip, attrs, description)
            VALUES (?, ?, ?, '', ?)
            "#,
        )
        .bind(GW_TYPE_CARRIER)
        .bind(&address)
        .bind(carrier.strip_digits.unwrap_or(0))
        .bind(&carrier.name)
        .execute(&kamailio_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to insert Kamailio gateway: {}", e)))?;

        gwid = result.last_insert_id() as i32;

        // attrs starts with the gwid, which is only known once the INSERT has assigned it
        sqlx::query("UPDATE dr_gateways SET attrs = ? WHERE gwid = ?")
            .bind(gateway_attrs(gwid, GW_TYPE_CARRIER, signalling))
            .bind(gwid)
            .execute(&kamailio_pool)
            .await
            .map_err(|e| AppError::Database(format!("Failed to set Kamailio gateway attrs: {}", e)))?;

        // Update PostgreSQL with the new gwid
        sqlx::query!(
            "UPDATE carriers SET kamailio_gwid = $1, sync_status = 'synced', sync_error = NULL WHERE id = $2",
            gwid,
            carrier_id
        )
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    }

    // Mark as synced
    sqlx::query!(
        "UPDATE carriers SET sync_status = 'synced', sync_error = NULL WHERE id = $1",
        carrier_id
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

/// Check SIP OPTIONS for a host
struct SipOptionsResult {
    status: SipStatus,
    latency_ms: Option<i32>,
    response_code: Option<i32>,
    error: Option<String>,
}

async fn check_sip_options(host: &str, port: i32) -> SipOptionsResult {
    use std::time::Instant;
    use tokio::process::Command;

    let target = format!("sip:{}:{}", host, port);
    let start = Instant::now();

    let output = Command::new("sipsak")
        .args(["-s", &target, "-v", "-W", "3"])
        .output()
        .await;

    match output {
        Ok(out) => {
            let elapsed = start.elapsed().as_millis() as i32;
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);

            if out.status.success() {
                // Parse response code from output
                let response_code = stdout
                    .lines()
                    .find(|l| l.contains("SIP/2.0"))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .and_then(|s| s.parse().ok());

                SipOptionsResult {
                    status: SipStatus::Reachable,
                    latency_ms: Some(elapsed),
                    response_code,
                    error: None,
                }
            } else {
                SipOptionsResult {
                    status: SipStatus::Unreachable,
                    latency_ms: Some(elapsed),
                    response_code: None,
                    error: Some(stderr.to_string()),
                }
            }
        }
        Err(e) => SipOptionsResult {
            status: SipStatus::Unreachable,
            latency_ms: None,
            response_code: None,
            error: Some(format!("Failed to execute sipsak: {}", e)),
        },
    }
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

/// Configure carriers routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/carriers")
            .route("", web::get().to(list_carriers))
            .route("", web::post().to(create_carrier))
            .route("/sync", web::post().to(sync_all_carriers))
            .route("/{id}", web::get().to(get_carrier))
            .route("/{id}", web::put().to(update_carrier))
            .route("/{id}", web::delete().to(delete_carrier))
            .route("/{id}/check-status", web::post().to(check_carrier_status)),
    );
}
