//! Unified Routing Handlers
//!
//! HTTP handlers for the unified routing management system.
//! Manages trunks, trunk groups, outbound routes, and inbound routes
//! with automatic synchronization to Kamailio and FreeSWITCH.

use crate::dto::routing::*;
use crate::dto::ApiResponse;
use actix_web::{web, HttpResponse};
use apolo_auth::SuperadminUser;
use apolo_core::models::AuditLogBuilder;
use apolo_core::AppError;
use apolo_services::SipDeviceService;
use serde_json::json;
use sqlx::mysql::MySqlPool;
use sqlx::PgPool;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

// FreeSWITCH dialplan file path
const TO_KAMAILIO_XML: &str = "/etc/freeswitch/dialplan/to-kamailio.xml";

// ============================================================================
// Trunk Handlers
// ============================================================================

/// List all trunks
///
/// GET /api/v1/routing/trunks
#[instrument(skip(pool, _admin))]
pub async fn list_trunks(
    pool: web::Data<PgPool>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!("Listing routing trunks");

    let trunks = sqlx::query_as!(
        Trunk,
        r#"
        SELECT id, name, description, host, port, transport,
               auth_username, auth_password_encrypted, auth_password_nonce, strip_digits,
               prefix_to_add, enabled, trunk_type, freeswitch_gateway_name,
               kamailio_gwid, sync_status, sync_error,
               sip_status, sip_status_message, last_options_check,
               last_options_latency_ms, last_options_response_code,
               created_at, updated_at
        FROM routing_trunks
        ORDER BY name
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to fetch trunks");
        AppError::Database(format!("Failed to fetch trunks: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(trunks)))
}

/// Get a single trunk
///
/// GET /api/v1/routing/trunks/{id}
#[instrument(skip(pool, _admin))]
pub async fn get_trunk(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let trunk_id = path.into_inner();
    info!(trunk_id = %trunk_id, "Getting trunk");

    let trunk = sqlx::query_as!(
        Trunk,
        r#"
        SELECT id, name, description, host, port, transport,
               auth_username, auth_password_encrypted, auth_password_nonce, strip_digits,
               prefix_to_add, enabled, trunk_type, freeswitch_gateway_name,
               kamailio_gwid, sync_status, sync_error,
               sip_status, sip_status_message, last_options_check,
               last_options_latency_ms, last_options_response_code,
               created_at, updated_at
        FROM routing_trunks
        WHERE id = $1
        "#,
        trunk_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to fetch trunk");
        AppError::Database(format!("Failed to fetch trunk: {}", e))
    })?;

    match trunk {
        Some(t) => Ok(HttpResponse::Ok().json(ApiResponse::success(t))),
        None => Err(AppError::NotFound(format!("Trunk {} not found", trunk_id))),
    }
}

/// Create a new trunk
///
/// POST /api/v1/routing/trunks
#[instrument(skip(pool, mysql_pool, sip_service, admin, req))]
pub async fn create_trunk(
    pool: web::Data<PgPool>,
    mysql_pool: Option<web::Data<MySqlPool>>,
    sip_service: Option<web::Data<Arc<SipDeviceService>>>,
    req: web::Json<CreateTrunkRequest>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!(name = %req.name, host = %req.host, "Creating trunk");

    // Validate request
    if req.name.trim().is_empty() {
        return Err(AppError::Validation("Trunk name cannot be empty".to_string()));
    }
    if req.host.trim().is_empty() {
        return Err(AppError::Validation("Host cannot be empty".to_string()));
    }

    let transport = if req.transport.is_empty() { "udp" } else { &req.transport };

    // Validate trunk_type
    let trunk_type = if req.trunk_type.is_empty() || (req.trunk_type != "private" && req.trunk_type != "public") {
        "public"
    } else {
        &req.trunk_type
    };

    // Encrypt password if provided and sip_service is available
    let (password_encrypted, password_nonce): (Option<Vec<u8>>, Option<Vec<u8>>) =
        if let Some(password) = &req.auth_password {
            if !password.is_empty() {
                if let Some(sip_svc) = &sip_service {
                    match sip_svc.encrypt_password(password) {
                        Ok((encrypted, nonce)) => (Some(encrypted), Some(nonce)),
                        Err(e) => {
                            warn!(error = %e, "Failed to encrypt password, storing without encryption");
                            (None, None)
                        }
                    }
                } else {
                    warn!("SIP service not available, cannot encrypt password");
                    (None, None)
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

    // Insert into PostgreSQL
    let trunk = sqlx::query_as!(
        Trunk,
        r#"
        INSERT INTO routing_trunks (name, description, host, port, transport,
                                    auth_username, auth_password_encrypted, auth_password_nonce,
                                    strip_digits, prefix_to_add, enabled, trunk_type)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING id, name, description, host, port, transport,
                  auth_username, auth_password_encrypted, auth_password_nonce, strip_digits,
                  prefix_to_add, enabled, trunk_type, freeswitch_gateway_name,
                  kamailio_gwid, sync_status, sync_error,
                  sip_status, sip_status_message, last_options_check,
                  last_options_latency_ms, last_options_response_code,
                  created_at, updated_at
        "#,
        req.name.trim(),
        req.description,
        req.host.trim(),
        req.port,
        transport,
        req.auth_username,
        password_encrypted.as_deref(),
        password_nonce.as_deref(),
        req.strip_digits,
        req.prefix_to_add,
        req.enabled,
        trunk_type
    )
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to create trunk");
        AppError::Database(format!("Failed to create trunk: {}", e))
    })?;

    // Sync based on trunk_type
    if trunk_type == "public" {
        // Public trunks -> Kamailio
        if let Some(mysql) = mysql_pool {
            match sync_trunk_to_kamailio(&pool, &mysql, &trunk).await {
                Ok(gwid) => {
                    // Update trunk with Kamailio gwid and sync status
                    let _ = sqlx::query!(
                        "UPDATE routing_trunks SET kamailio_gwid = $1, sync_status = 'synced' WHERE id = $2",
                        gwid,
                        trunk.id
                    )
                    .execute(pool.get_ref())
                    .await;

                    log_sync_operation(&pool, "create", "trunk", Some(trunk.id), "kamailio", "success", None).await;
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    let _ = sqlx::query!(
                        "UPDATE routing_trunks SET sync_status = 'error', sync_error = $1 WHERE id = $2",
                        &error_msg,
                        trunk.id
                    )
                    .execute(pool.get_ref())
                    .await;

                    log_sync_operation(&pool, "create", "trunk", Some(trunk.id), "kamailio", "error", Some(&error_msg)).await;
                    warn!(error = %e, "Failed to sync trunk to Kamailio");
                }
            }
        }
    } else {
        // Private trunks -> FreeSWITCH
        let gateway_name = slugify(&trunk.name);
        let sip_svc_ref = sip_service.as_ref().map(|s| s.get_ref());
        match sync_trunk_to_freeswitch(&pool, &trunk, &gateway_name, sip_svc_ref).await {
            Ok(_) => {
                let _ = sqlx::query!(
                    "UPDATE routing_trunks SET freeswitch_gateway_name = $1, sync_status = 'synced' WHERE id = $2",
                    gateway_name,
                    trunk.id
                )
                .execute(pool.get_ref())
                .await;

                log_sync_operation(&pool, "create", "trunk", Some(trunk.id), "freeswitch", "success", None).await;
            }
            Err(e) => {
                let error_msg = e.to_string();
                let _ = sqlx::query!(
                    "UPDATE routing_trunks SET sync_status = 'error', sync_error = $1 WHERE id = $2",
                    &error_msg,
                    trunk.id
                )
                .execute(pool.get_ref())
                .await;

                log_sync_operation(&pool, "create", "trunk", Some(trunk.id), "freeswitch", "error", Some(&error_msg)).await;
                warn!(error = %e, "Failed to sync trunk to FreeSWITCH");
            }
        }
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("create_trunk")
        .entity_type("routing_trunk")
        .entity_id(trunk.id.to_string())
        .details(json!({ "name": trunk.name, "host": trunk.host }))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Created().json(ApiResponse::success(trunk)))
}

/// Update a trunk
///
/// PUT /api/v1/routing/trunks/{id}
#[instrument(skip(pool, mysql_pool, sip_service, admin, req))]
pub async fn update_trunk(
    pool: web::Data<PgPool>,
    mysql_pool: Option<web::Data<MySqlPool>>,
    sip_service: Option<web::Data<Arc<SipDeviceService>>>,
    path: web::Path<Uuid>,
    req: web::Json<UpdateTrunkRequest>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let trunk_id = path.into_inner();
    info!(trunk_id = %trunk_id, "Updating trunk");

    // Check trunk exists
    let existing = sqlx::query!(
        "SELECT id, trunk_type, kamailio_gwid, freeswitch_gateway_name FROM routing_trunks WHERE id = $1",
        trunk_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let existing = existing.ok_or_else(|| AppError::NotFound(format!("Trunk {} not found", trunk_id)))?;

    // Encrypt password if provided
    let (password_encrypted, password_nonce): (Option<Vec<u8>>, Option<Vec<u8>>) =
        if let Some(password) = &req.auth_password {
            if !password.is_empty() {
                if let Some(sip_svc) = &sip_service {
                    match sip_svc.encrypt_password(password) {
                        Ok((encrypted, nonce)) => (Some(encrypted), Some(nonce)),
                        Err(e) => {
                            warn!(error = %e, "Failed to encrypt password");
                            (None, None)
                        }
                    }
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

    // Build dynamic update - handle password separately to allow NULL coalescing
    let trunk = sqlx::query_as!(
        Trunk,
        r#"
        UPDATE routing_trunks SET
            name = COALESCE($1, name),
            description = COALESCE($2, description),
            host = COALESCE($3, host),
            port = COALESCE($4, port),
            transport = COALESCE($5, transport),
            auth_username = COALESCE($6, auth_username),
            auth_password_encrypted = COALESCE($7, auth_password_encrypted),
            auth_password_nonce = COALESCE($8, auth_password_nonce),
            strip_digits = COALESCE($9, strip_digits),
            prefix_to_add = COALESCE($10, prefix_to_add),
            enabled = COALESCE($11, enabled),
            trunk_type = COALESCE($12, trunk_type),
            sync_status = 'pending'
        WHERE id = $13
        RETURNING id, name, description, host, port, transport,
                  auth_username, auth_password_encrypted, auth_password_nonce, strip_digits,
                  prefix_to_add, enabled, trunk_type, freeswitch_gateway_name,
                  kamailio_gwid, sync_status, sync_error,
                  sip_status, sip_status_message, last_options_check,
                  last_options_latency_ms, last_options_response_code,
                  created_at, updated_at
        "#,
        req.name.as_deref(),
        req.description,
        req.host.as_deref(),
        req.port,
        req.transport.as_deref(),
        req.auth_username,
        password_encrypted.as_deref(),
        password_nonce.as_deref(),
        req.strip_digits,
        req.prefix_to_add.as_deref(),
        req.enabled,
        req.trunk_type.as_deref(),
        trunk_id
    )
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to update trunk");
        AppError::Database(format!("Failed to update trunk: {}", e))
    })?;

    // Sync based on trunk_type
    let current_trunk_type = trunk.trunk_type.as_deref().unwrap_or("public");

    if current_trunk_type == "public" {
        // Public trunks -> Kamailio
        if let Some(mysql) = mysql_pool {
            if let Some(gwid) = existing.kamailio_gwid {
                match update_trunk_in_kamailio(&mysql, gwid, &trunk).await {
                    Ok(_) => {
                        let _ = sqlx::query!(
                            "UPDATE routing_trunks SET sync_status = 'synced', sync_error = NULL WHERE id = $1",
                            trunk.id
                        )
                        .execute(pool.get_ref())
                        .await;

                        log_sync_operation(&pool, "update", "trunk", Some(trunk.id), "kamailio", "success", None).await;
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        let _ = sqlx::query!(
                            "UPDATE routing_trunks SET sync_status = 'error', sync_error = $1 WHERE id = $2",
                            &error_msg,
                            trunk.id
                        )
                        .execute(pool.get_ref())
                        .await;

                        log_sync_operation(&pool, "update", "trunk", Some(trunk.id), "kamailio", "error", Some(&error_msg)).await;
                    }
                }
            } else {
                // Create new gateway in Kamailio
                match sync_trunk_to_kamailio(&pool, &mysql, &trunk).await {
                    Ok(gwid) => {
                        let _ = sqlx::query!(
                            "UPDATE routing_trunks SET kamailio_gwid = $1, sync_status = 'synced' WHERE id = $2",
                            gwid,
                            trunk.id
                        )
                        .execute(pool.get_ref())
                        .await;
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to sync trunk to Kamailio");
                    }
                }
            }
        }
    } else {
        // Private trunks -> FreeSWITCH
        let gateway_name = existing.freeswitch_gateway_name.unwrap_or_else(|| slugify(&trunk.name));
        let sip_svc_ref = sip_service.as_ref().map(|s| s.get_ref());
        match sync_trunk_to_freeswitch(&pool, &trunk, &gateway_name, sip_svc_ref).await {
            Ok(_) => {
                let _ = sqlx::query!(
                    "UPDATE routing_trunks SET freeswitch_gateway_name = $1, sync_status = 'synced', sync_error = NULL WHERE id = $2",
                    gateway_name,
                    trunk.id
                )
                .execute(pool.get_ref())
                .await;

                log_sync_operation(&pool, "update", "trunk", Some(trunk.id), "freeswitch", "success", None).await;
            }
            Err(e) => {
                let error_msg = e.to_string();
                let _ = sqlx::query!(
                    "UPDATE routing_trunks SET sync_status = 'error', sync_error = $1 WHERE id = $2",
                    &error_msg,
                    trunk.id
                )
                .execute(pool.get_ref())
                .await;

                log_sync_operation(&pool, "update", "trunk", Some(trunk.id), "freeswitch", "error", Some(&error_msg)).await;
            }
        }
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("update_trunk")
        .entity_type("routing_trunk")
        .entity_id(trunk.id.to_string())
        .details(json!({ "name": trunk.name, "trunk_type": current_trunk_type }))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(trunk)))
}

/// Delete a trunk
///
/// DELETE /api/v1/routing/trunks/{id}
#[instrument(skip(pool, mysql_pool, admin))]
pub async fn delete_trunk(
    pool: web::Data<PgPool>,
    mysql_pool: Option<web::Data<MySqlPool>>,
    path: web::Path<Uuid>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let trunk_id = path.into_inner();
    info!(trunk_id = %trunk_id, "Deleting trunk");

    // Get trunk info for deletion
    let existing = sqlx::query!(
        "SELECT kamailio_gwid, name, trunk_type, freeswitch_gateway_name FROM routing_trunks WHERE id = $1",
        trunk_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let existing = existing.ok_or_else(|| AppError::NotFound(format!("Trunk {} not found", trunk_id)))?;

    // Check if trunk is used by any inbound routes (as destination_trunk_id)
    let inbound_routes_using_trunk = sqlx::query!(
        r#"SELECT name FROM routing_inbound_routes WHERE destination_trunk_id = $1"#,
        trunk_id
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    if !inbound_routes_using_trunk.is_empty() {
        let route_names: Vec<String> = inbound_routes_using_trunk.iter().map(|r| r.name.clone()).collect();
        return Err(AppError::Validation(format!(
            "No se puede eliminar la troncal '{}' porque está siendo usada por las siguientes rutas entrantes: {}. Elimina o modifica estas rutas primero.",
            existing.name,
            route_names.join(", ")
        )));
    }

    // Check if trunk is used by any outbound routes
    let outbound_routes_using_trunk = sqlx::query!(
        r#"SELECT name FROM routing_outbound_routes WHERE trunk_id = $1"#,
        trunk_id
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    if !outbound_routes_using_trunk.is_empty() {
        let route_names: Vec<String> = outbound_routes_using_trunk.iter().map(|r| r.name.clone()).collect();
        return Err(AppError::Validation(format!(
            "No se puede eliminar la troncal '{}' porque está siendo usada por las siguientes rutas salientes: {}. Elimina o modifica estas rutas primero.",
            existing.name,
            route_names.join(", ")
        )));
    }

    // Delete based on trunk_type
    let trunk_type = existing.trunk_type.as_deref().unwrap_or("public");

    if trunk_type == "public" {
        // Delete from Kamailio MySQL (with timeout to prevent blocking)
        if let Some(mysql) = mysql_pool {
            if let Some(gwid) = existing.kamailio_gwid {
                let delete_future = delete_trunk_from_kamailio(&mysql, gwid);
                match tokio::time::timeout(std::time::Duration::from_secs(5), delete_future).await {
                    Ok(Ok(())) => {
                        info!(gwid = gwid, "Trunk deleted from Kamailio");
                        log_sync_operation(&pool, "delete", "trunk", Some(trunk_id), "kamailio", "success", None).await;
                    }
                    Ok(Err(e)) => {
                        warn!(error = %e, gwid = gwid, "Failed to delete trunk from Kamailio");
                        log_sync_operation(&pool, "delete", "trunk", Some(trunk_id), "kamailio", "error", Some(&e.to_string())).await;
                    }
                    Err(_) => {
                        warn!(gwid = gwid, "Timeout deleting trunk from Kamailio");
                        log_sync_operation(&pool, "delete", "trunk", Some(trunk_id), "kamailio", "error", Some("Timeout")).await;
                    }
                }
            }
        }
    } else {
        // Delete FreeSWITCH gateway for private trunks
        if let Some(gateway_name) = &existing.freeswitch_gateway_name {
            match delete_trunk_from_freeswitch(gateway_name).await {
                Ok(_) => {
                    info!(gateway = %gateway_name, "Gateway deleted from FreeSWITCH");
                    log_sync_operation(&pool, "delete", "trunk", Some(trunk_id), "freeswitch", "success", None).await;
                }
                Err(e) => {
                    warn!(error = %e, gateway = %gateway_name, "Failed to delete gateway from FreeSWITCH");
                    log_sync_operation(&pool, "delete", "trunk", Some(trunk_id), "freeswitch", "error", Some(&e.to_string())).await;
                }
            }
        }
    }

    // Delete from PostgreSQL (trunk_group_members will cascade automatically)
    let result = sqlx::query!("DELETE FROM routing_trunks WHERE id = $1", trunk_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Trunk {} not found", trunk_id)));
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("delete_trunk")
        .entity_type("routing_trunk")
        .entity_id(trunk_id.to_string())
        .details(json!({}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        json!({ "deleted": trunk_id }),
        "Trunk deleted successfully",
    )))
}

// ============================================================================
// Trunk Group Handlers
// ============================================================================

/// List all trunk groups
///
/// GET /api/v1/routing/trunk-groups
#[instrument(skip(pool, _admin))]
pub async fn list_trunk_groups(
    pool: web::Data<PgPool>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!("Listing trunk groups");

    let groups = sqlx::query_as!(
        TrunkGroup,
        r#"
        SELECT id, name, description, failover_strategy,
               kamailio_group_id, sync_status, sync_error,
               created_at, updated_at
        FROM routing_trunk_groups
        ORDER BY name
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(groups)))
}

/// Get a trunk group with its members
///
/// GET /api/v1/routing/trunk-groups/{id}
#[instrument(skip(pool, _admin))]
pub async fn get_trunk_group(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let group_id = path.into_inner();
    info!(group_id = %group_id, "Getting trunk group");

    let group = sqlx::query_as!(
        TrunkGroup,
        r#"
        SELECT id, name, description, failover_strategy,
               kamailio_group_id, sync_status, sync_error,
               created_at, updated_at
        FROM routing_trunk_groups
        WHERE id = $1
        "#,
        group_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let group = group.ok_or_else(|| AppError::NotFound(format!("Trunk group {} not found", group_id)))?;

    // Get members with trunk details
    let members = sqlx::query_as!(
        TrunkGroupMember,
        r#"
        SELECT m.id, m.trunk_id, t.name as trunk_name, t.host as trunk_host,
               t.port as trunk_port, t.enabled as trunk_enabled,
               m.priority, m.weight, m.max_channels
        FROM routing_trunk_group_members m
        JOIN routing_trunks t ON t.id = m.trunk_id
        WHERE m.group_id = $1
        ORDER BY m.priority, t.name
        "#,
        group_id
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let result = TrunkGroupWithMembers { group, members };
    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

/// Create a trunk group
///
/// POST /api/v1/routing/trunk-groups
#[instrument(skip(pool, mysql_pool, admin, req))]
pub async fn create_trunk_group(
    pool: web::Data<PgPool>,
    mysql_pool: Option<web::Data<MySqlPool>>,
    req: web::Json<CreateTrunkGroupRequest>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!(name = %req.name, "Creating trunk group");

    if req.name.trim().is_empty() {
        return Err(AppError::Validation("Group name cannot be empty".to_string()));
    }

    // Start transaction
    let mut tx = pool.begin().await.map_err(|e| AppError::Database(e.to_string()))?;

    // Create group
    let group = sqlx::query_as!(
        TrunkGroup,
        r#"
        INSERT INTO routing_trunk_groups (name, description, failover_strategy)
        VALUES ($1, $2, $3)
        RETURNING id, name, description, failover_strategy,
                  kamailio_group_id, sync_status, sync_error,
                  created_at, updated_at
        "#,
        req.name.trim(),
        req.description,
        req.failover_strategy
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Add members
    for member in &req.members {
        sqlx::query!(
            r#"
            INSERT INTO routing_trunk_group_members (group_id, trunk_id, priority, weight, max_channels)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            group.id,
            member.trunk_id,
            member.priority,
            member.weight,
            member.max_channels
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Database(format!("Failed to add member: {}", e)))?;
    }

    tx.commit().await.map_err(|e| AppError::Database(e.to_string()))?;

    // Sync to Kamailio
    if let Some(mysql) = mysql_pool {
        let members = sqlx::query!(
            "SELECT trunk_id FROM routing_trunk_group_members WHERE group_id = $1 ORDER BY priority",
            group.id
        )
        .fetch_all(pool.get_ref())
        .await
        .ok();

        if let Some(members) = members {
            let trunk_ids: Vec<Uuid> = members.iter().map(|m| m.trunk_id).collect();
            match sync_trunk_group_to_kamailio(&pool, &mysql, &group, &trunk_ids).await {
                Ok(group_id) => {
                    let _ = sqlx::query!(
                        "UPDATE routing_trunk_groups SET kamailio_group_id = $1, sync_status = 'synced' WHERE id = $2",
                        group_id,
                        group.id
                    )
                    .execute(pool.get_ref())
                    .await;

                    log_sync_operation(&pool, "create", "trunk_group", Some(group.id), "kamailio", "success", None).await;
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    let _ = sqlx::query!(
                        "UPDATE routing_trunk_groups SET sync_status = 'error', sync_error = $1 WHERE id = $2",
                        &error_msg,
                        group.id
                    )
                    .execute(pool.get_ref())
                    .await;

                    log_sync_operation(&pool, "create", "trunk_group", Some(group.id), "kamailio", "error", Some(&error_msg)).await;
                }
            }
        }
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("create_trunk_group")
        .entity_type("routing_trunk_group")
        .entity_id(group.id.to_string())
        .details(json!({ "name": group.name }))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Created().json(ApiResponse::success(group)))
}

/// Update a trunk group
///
/// PUT /api/v1/routing/trunk-groups/{id}
#[instrument(skip(pool, mysql_pool, admin, req))]
pub async fn update_trunk_group(
    pool: web::Data<PgPool>,
    mysql_pool: Option<web::Data<MySqlPool>>,
    path: web::Path<Uuid>,
    req: web::Json<UpdateTrunkGroupRequest>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let group_id = path.into_inner();
    info!(group_id = %group_id, "Updating trunk group");

    let mut tx = pool.begin().await.map_err(|e| AppError::Database(e.to_string()))?;

    // Update group
    let group = sqlx::query_as!(
        TrunkGroup,
        r#"
        UPDATE routing_trunk_groups SET
            name = COALESCE($1, name),
            description = COALESCE($2, description),
            failover_strategy = COALESCE($3, failover_strategy),
            sync_status = 'pending'
        WHERE id = $4
        RETURNING id, name, description, failover_strategy,
                  kamailio_group_id, sync_status, sync_error,
                  created_at, updated_at
        "#,
        req.name.as_deref(),
        req.description,
        req.failover_strategy.as_deref(),
        group_id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let group = group.ok_or_else(|| AppError::NotFound(format!("Trunk group {} not found", group_id)))?;

    // Update members if provided
    if let Some(members) = &req.members {
        // Remove existing members
        sqlx::query!("DELETE FROM routing_trunk_group_members WHERE group_id = $1", group_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        // Add new members
        for member in members {
            sqlx::query!(
                r#"
                INSERT INTO routing_trunk_group_members (group_id, trunk_id, priority, weight, max_channels)
                VALUES ($1, $2, $3, $4, $5)
                "#,
                group_id,
                member.trunk_id,
                member.priority,
                member.weight,
                member.max_channels
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Database(format!("Failed to add member: {}", e)))?;
        }
    }

    tx.commit().await.map_err(|e| AppError::Database(e.to_string()))?;

    // Sync to Kamailio
    if let Some(mysql) = mysql_pool {
        let members = sqlx::query!(
            "SELECT trunk_id FROM routing_trunk_group_members WHERE group_id = $1 ORDER BY priority",
            group.id
        )
        .fetch_all(pool.get_ref())
        .await
        .ok();

        if let Some(members) = members {
            let trunk_ids: Vec<Uuid> = members.iter().map(|m| m.trunk_id).collect();

            if let Some(kam_group_id) = group.kamailio_group_id {
                match update_trunk_group_in_kamailio(&pool, &mysql, kam_group_id, &group, &trunk_ids).await {
                    Ok(_) => {
                        let _ = sqlx::query!(
                            "UPDATE routing_trunk_groups SET sync_status = 'synced', sync_error = NULL WHERE id = $1",
                            group.id
                        )
                        .execute(pool.get_ref())
                        .await;

                        log_sync_operation(&pool, "update", "trunk_group", Some(group.id), "kamailio", "success", None).await;
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        let _ = sqlx::query!(
                            "UPDATE routing_trunk_groups SET sync_status = 'error', sync_error = $1 WHERE id = $2",
                            &error_msg,
                            group.id
                        )
                        .execute(pool.get_ref())
                        .await;

                        log_sync_operation(&pool, "update", "trunk_group", Some(group.id), "kamailio", "error", Some(&error_msg)).await;
                    }
                }
            }
        }
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("update_trunk_group")
        .entity_type("routing_trunk_group")
        .entity_id(group.id.to_string())
        .details(json!({ "name": group.name }))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(group)))
}

/// Delete a trunk group
///
/// DELETE /api/v1/routing/trunk-groups/{id}
#[instrument(skip(pool, mysql_pool, admin))]
pub async fn delete_trunk_group(
    pool: web::Data<PgPool>,
    mysql_pool: Option<web::Data<MySqlPool>>,
    path: web::Path<Uuid>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let group_id = path.into_inner();
    info!(group_id = %group_id, "Deleting trunk group");

    // Get Kamailio group ID for deletion
    let existing = sqlx::query!("SELECT kamailio_group_id FROM routing_trunk_groups WHERE id = $1", group_id)
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let existing = existing.ok_or_else(|| AppError::NotFound(format!("Trunk group {} not found", group_id)))?;

    // Delete from Kamailio first
    if let Some(mysql) = mysql_pool {
        if let Some(kam_id) = existing.kamailio_group_id {
            if let Err(e) = delete_trunk_group_from_kamailio(&mysql, kam_id).await {
                warn!(error = %e, "Failed to delete trunk group from Kamailio");
            } else {
                log_sync_operation(&pool, "delete", "trunk_group", Some(group_id), "kamailio", "success", None).await;
            }
        }
    }

    // Delete from PostgreSQL (cascade deletes members)
    let result = sqlx::query!("DELETE FROM routing_trunk_groups WHERE id = $1", group_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Trunk group {} not found", group_id)));
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("delete_trunk_group")
        .entity_type("routing_trunk_group")
        .entity_id(group_id.to_string())
        .details(json!({}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        json!({ "deleted": group_id }),
        "Trunk group deleted successfully",
    )))
}

// ============================================================================
// Outbound Route Handlers
// ============================================================================

/// List all outbound routes
///
/// GET /api/v1/routing/outbound
#[instrument(skip(pool, _admin))]
pub async fn list_outbound_routes(
    pool: web::Data<PgPool>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!("Listing outbound routes");

    let routes = sqlx::query_as!(
        OutboundRoute,
        r#"
        SELECT r.id, r.name, r.description, r.prefix_pattern, r.priority,
               r.trunk_group_id, g.name as "trunk_group_name?",
               r.trunk_id, t.name as "trunk_name?",
               r.time_schedule, r.time_schedule_enabled, r.enabled,
               r.kamailio_ruleid, r.sync_status, r.sync_error,
               r.created_at, r.updated_at
        FROM routing_outbound_routes r
        LEFT JOIN routing_trunk_groups g ON g.id = r.trunk_group_id
        LEFT JOIN routing_trunks t ON t.id = r.trunk_id
        ORDER BY r.prefix_pattern, r.priority
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(routes)))
}

/// Get a single outbound route
///
/// GET /api/v1/routing/outbound/{id}
#[instrument(skip(pool, _admin))]
pub async fn get_outbound_route(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let route_id = path.into_inner();

    let route = sqlx::query_as!(
        OutboundRoute,
        r#"
        SELECT r.id, r.name, r.description, r.prefix_pattern, r.priority,
               r.trunk_group_id, g.name as "trunk_group_name?",
               r.trunk_id, t.name as "trunk_name?",
               r.time_schedule, r.time_schedule_enabled, r.enabled,
               r.kamailio_ruleid, r.sync_status, r.sync_error,
               r.created_at, r.updated_at
        FROM routing_outbound_routes r
        LEFT JOIN routing_trunk_groups g ON g.id = r.trunk_group_id
        LEFT JOIN routing_trunks t ON t.id = r.trunk_id
        WHERE r.id = $1
        "#,
        route_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    match route {
        Some(r) => Ok(HttpResponse::Ok().json(ApiResponse::success(r))),
        None => Err(AppError::NotFound(format!("Outbound route {} not found", route_id))),
    }
}

/// Create an outbound route
///
/// POST /api/v1/routing/outbound
#[instrument(skip(pool, mysql_pool, admin, req))]
pub async fn create_outbound_route(
    pool: web::Data<PgPool>,
    mysql_pool: Option<web::Data<MySqlPool>>,
    req: web::Json<CreateOutboundRouteRequest>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!(name = %req.name, prefix = %req.prefix_pattern, "Creating outbound route");

    // Validate
    if req.name.trim().is_empty() {
        return Err(AppError::Validation("Route name cannot be empty".to_string()));
    }
    if req.trunk_group_id.is_none() && req.trunk_id.is_none() {
        return Err(AppError::Validation("Must specify either trunk_group_id or trunk_id".to_string()));
    }
    if req.trunk_group_id.is_some() && req.trunk_id.is_some() {
        return Err(AppError::Validation("Cannot specify both trunk_group_id and trunk_id".to_string()));
    }

    let route = sqlx::query_as!(
        OutboundRoute,
        r#"
        INSERT INTO routing_outbound_routes (name, description, prefix_pattern, priority,
                                            trunk_group_id, trunk_id, time_schedule,
                                            time_schedule_enabled, enabled)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id, name, description, prefix_pattern, priority,
                  trunk_group_id, NULL as trunk_group_name,
                  trunk_id, NULL as trunk_name,
                  time_schedule, time_schedule_enabled, enabled,
                  kamailio_ruleid, sync_status, sync_error,
                  created_at, updated_at
        "#,
        req.name.trim(),
        req.description,
        req.prefix_pattern.trim(),
        req.priority,
        req.trunk_group_id,
        req.trunk_id,
        req.time_schedule,
        req.time_schedule_enabled,
        req.enabled
    )
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Sync to Kamailio immediately
    if let Some(mysql) = mysql_pool {
        match sync_outbound_route_to_kamailio(&pool, &mysql, &route).await {
            Ok(ruleid) => {
                let _ = sqlx::query!(
                    "UPDATE routing_outbound_routes SET kamailio_ruleid = $1, sync_status = 'synced' WHERE id = $2",
                    ruleid,
                    route.id
                )
                .execute(pool.get_ref())
                .await;

                // Reload Kamailio to apply changes
                if let Err(e) = reload_kamailio().await {
                    warn!("Failed to reload Kamailio after creating route: {}", e);
                }

                log_sync_operation(&pool, "create", "outbound_route", Some(route.id), "kamailio", "success", None).await;
            }
            Err(e) => {
                let error_msg = e.to_string();
                let _ = sqlx::query!(
                    "UPDATE routing_outbound_routes SET sync_status = 'error', sync_error = $1 WHERE id = $2",
                    &error_msg,
                    route.id
                )
                .execute(pool.get_ref())
                .await;

                log_sync_operation(&pool, "create", "outbound_route", Some(route.id), "kamailio", "error", Some(&error_msg)).await;
            }
        }
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("create_outbound_route")
        .entity_type("routing_outbound_route")
        .entity_id(route.id.to_string())
        .details(json!({ "name": route.name, "prefix": route.prefix_pattern }))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Created().json(ApiResponse::success(route)))
}

/// Update an outbound route
///
/// PUT /api/v1/routing/outbound/{id}
#[instrument(skip(pool, mysql_pool, admin, req))]
pub async fn update_outbound_route(
    pool: web::Data<PgPool>,
    mysql_pool: Option<web::Data<MySqlPool>>,
    path: web::Path<Uuid>,
    req: web::Json<UpdateOutboundRouteRequest>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let route_id = path.into_inner();
    info!(route_id = %route_id, "Updating outbound route");

    // Validate mutual exclusivity
    if req.trunk_group_id.is_some() && req.trunk_id.is_some() {
        return Err(AppError::Validation("Cannot specify both trunk_group_id and trunk_id".to_string()));
    }

    let route = sqlx::query_as!(
        OutboundRoute,
        r#"
        UPDATE routing_outbound_routes SET
            name = COALESCE($1, name),
            description = COALESCE($2, description),
            prefix_pattern = COALESCE($3, prefix_pattern),
            priority = COALESCE($4, priority),
            trunk_group_id = CASE WHEN $5::uuid IS NOT NULL THEN $5 ELSE trunk_group_id END,
            trunk_id = CASE WHEN $6::uuid IS NOT NULL THEN $6 ELSE trunk_id END,
            time_schedule = COALESCE($7, time_schedule),
            time_schedule_enabled = COALESCE($8, time_schedule_enabled),
            enabled = COALESCE($9, enabled),
            sync_status = 'pending'
        WHERE id = $10
        RETURNING id, name, description, prefix_pattern, priority,
                  trunk_group_id, NULL as trunk_group_name,
                  trunk_id, NULL as trunk_name,
                  time_schedule, time_schedule_enabled, enabled,
                  kamailio_ruleid, sync_status, sync_error,
                  created_at, updated_at
        "#,
        req.name.as_deref(),
        req.description,
        req.prefix_pattern.as_deref(),
        req.priority,
        req.trunk_group_id,
        req.trunk_id,
        req.time_schedule,
        req.time_schedule_enabled,
        req.enabled,
        route_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let route = route.ok_or_else(|| AppError::NotFound(format!("Outbound route {} not found", route_id)))?;

    // Sync to Kamailio immediately
    if let Some(mysql) = mysql_pool {
        if let Some(ruleid) = route.kamailio_ruleid {
            // Update existing rule in Kamailio
            match update_outbound_route_in_kamailio(&pool, &mysql, ruleid, &route).await {
                Ok(_) => {
                    let _ = sqlx::query!(
                        "UPDATE routing_outbound_routes SET sync_status = 'synced', sync_error = NULL WHERE id = $1",
                        route.id
                    )
                    .execute(pool.get_ref())
                    .await;

                    // Reload Kamailio to apply changes
                    if let Err(e) = reload_kamailio().await {
                        warn!("Failed to reload Kamailio after updating route: {}", e);
                    }

                    log_sync_operation(&pool, "update", "outbound_route", Some(route.id), "kamailio", "success", None).await;
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    let _ = sqlx::query!(
                        "UPDATE routing_outbound_routes SET sync_status = 'error', sync_error = $1 WHERE id = $2",
                        &error_msg,
                        route.id
                    )
                    .execute(pool.get_ref())
                    .await;

                    log_sync_operation(&pool, "update", "outbound_route", Some(route.id), "kamailio", "error", Some(&error_msg)).await;
                }
            }
        } else {
            // Route doesn't exist in Kamailio yet, create it
            match sync_outbound_route_to_kamailio(&pool, &mysql, &route).await {
                Ok(new_ruleid) => {
                    let _ = sqlx::query!(
                        "UPDATE routing_outbound_routes SET kamailio_ruleid = $1, sync_status = 'synced', sync_error = NULL WHERE id = $2",
                        new_ruleid,
                        route.id
                    )
                    .execute(pool.get_ref())
                    .await;

                    // Reload Kamailio to apply changes
                    if let Err(e) = reload_kamailio().await {
                        warn!("Failed to reload Kamailio after creating route: {}", e);
                    }

                    log_sync_operation(&pool, "update", "outbound_route", Some(route.id), "kamailio", "success", None).await;
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    let _ = sqlx::query!(
                        "UPDATE routing_outbound_routes SET sync_status = 'error', sync_error = $1 WHERE id = $2",
                        &error_msg,
                        route.id
                    )
                    .execute(pool.get_ref())
                    .await;

                    log_sync_operation(&pool, "update", "outbound_route", Some(route.id), "kamailio", "error", Some(&error_msg)).await;
                }
            }
        }
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("update_outbound_route")
        .entity_type("routing_outbound_route")
        .entity_id(route.id.to_string())
        .details(json!({ "name": route.name }))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(route)))
}

/// Delete an outbound route
///
/// DELETE /api/v1/routing/outbound/{id}
#[instrument(skip(pool, mysql_pool, admin))]
pub async fn delete_outbound_route(
    pool: web::Data<PgPool>,
    mysql_pool: Option<web::Data<MySqlPool>>,
    path: web::Path<Uuid>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let route_id = path.into_inner();
    info!(route_id = %route_id, "Deleting outbound route");

    let existing = sqlx::query!("SELECT kamailio_ruleid FROM routing_outbound_routes WHERE id = $1", route_id)
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let existing = existing.ok_or_else(|| AppError::NotFound(format!("Outbound route {} not found", route_id)))?;

    // Delete from Kamailio
    if let Some(mysql) = mysql_pool {
        if let Some(ruleid) = existing.kamailio_ruleid {
            if let Err(e) = delete_outbound_route_from_kamailio(&mysql, ruleid).await {
                warn!(error = %e, "Failed to delete outbound route from Kamailio");
            } else {
                // Reload Kamailio to apply changes
                if let Err(e) = reload_kamailio().await {
                    warn!("Failed to reload Kamailio after deleting route: {}", e);
                }
                log_sync_operation(&pool, "delete", "outbound_route", Some(route_id), "kamailio", "success", None).await;
            }
        }
    }

    let result = sqlx::query!("DELETE FROM routing_outbound_routes WHERE id = $1", route_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Outbound route {} not found", route_id)));
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("delete_outbound_route")
        .entity_type("routing_outbound_route")
        .entity_id(route_id.to_string())
        .details(json!({}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        json!({ "deleted": route_id }),
        "Outbound route deleted successfully",
    )))
}

// ============================================================================
// Inbound Route Handlers
// ============================================================================

/// List all inbound routes
///
/// GET /api/v1/routing/inbound
#[instrument(skip(pool, _admin))]
pub async fn list_inbound_routes(
    pool: web::Data<PgPool>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!("Listing inbound routes");

    let routes = sqlx::query!(
        r#"
        SELECT r.id, r.name, r.description, r.did_pattern, r.source_ip_pattern, r.priority,
               r.destination_host, r.destination_port, r.destination_profile,
               r.call_timeout, r.inherit_codec, r.ignore_early_media, r.bypass_media,
               r.strip_digits, r.prefix_to_add, r.destination_trunk_id,
               t.name as "destination_trunk_name?",
               r.send_early_media, r.failover_destinations, r.enabled, r.freeswitch_extension_id,
               r.sync_status, r.sync_error, r.created_at, r.updated_at
        FROM routing_inbound_routes r
        LEFT JOIN routing_trunks t ON r.destination_trunk_id = t.id
        ORDER BY r.priority, r.did_pattern
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let routes: Vec<InboundRoute> = routes
        .into_iter()
        .map(|r| InboundRoute {
            id: r.id,
            name: r.name,
            description: r.description,
            did_pattern: r.did_pattern,
            source_ip_pattern: r.source_ip_pattern,
            priority: r.priority,
            destination_host: r.destination_host,
            destination_port: r.destination_port,
            destination_profile: r.destination_profile,
            call_timeout: r.call_timeout,
            inherit_codec: r.inherit_codec,
            ignore_early_media: r.ignore_early_media,
            bypass_media: r.bypass_media,
            strip_digits: r.strip_digits,
            prefix_to_add: r.prefix_to_add,
            destination_trunk_id: r.destination_trunk_id,
            destination_trunk_name: r.destination_trunk_name,
            send_early_media: Some(r.send_early_media),
            failover_destinations: serde_json::from_value(r.failover_destinations.unwrap_or_default()).unwrap_or_default(),
            enabled: r.enabled,
            freeswitch_extension_id: r.freeswitch_extension_id,
            sync_status: r.sync_status,
            sync_error: r.sync_error,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect();

    Ok(HttpResponse::Ok().json(ApiResponse::success(routes)))
}

/// Get a single inbound route
///
/// GET /api/v1/routing/inbound/{id}
#[instrument(skip(pool, _admin))]
pub async fn get_inbound_route(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let route_id = path.into_inner();

    let r = sqlx::query!(
        r#"
        SELECT r.id, r.name, r.description, r.did_pattern, r.source_ip_pattern, r.priority,
               r.destination_host, r.destination_port, r.destination_profile,
               r.call_timeout, r.inherit_codec, r.ignore_early_media, r.bypass_media,
               r.strip_digits, r.prefix_to_add, r.destination_trunk_id,
               t.name as "destination_trunk_name?",
               r.send_early_media, r.failover_destinations, r.enabled, r.freeswitch_extension_id,
               r.sync_status, r.sync_error, r.created_at, r.updated_at
        FROM routing_inbound_routes r
        LEFT JOIN routing_trunks t ON r.destination_trunk_id = t.id
        WHERE r.id = $1
        "#,
        route_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    match r {
        Some(r) => {
            let route = InboundRoute {
                id: r.id,
                name: r.name,
                description: r.description,
                did_pattern: r.did_pattern,
                source_ip_pattern: r.source_ip_pattern,
                priority: r.priority,
                destination_host: r.destination_host,
                destination_port: r.destination_port,
                destination_profile: r.destination_profile,
                call_timeout: r.call_timeout,
                inherit_codec: r.inherit_codec,
                ignore_early_media: r.ignore_early_media,
                bypass_media: r.bypass_media,
                strip_digits: r.strip_digits,
                prefix_to_add: r.prefix_to_add,
                destination_trunk_id: r.destination_trunk_id,
                destination_trunk_name: r.destination_trunk_name,
                send_early_media: Some(r.send_early_media),
                failover_destinations: serde_json::from_value(r.failover_destinations.unwrap_or_default()).unwrap_or_default(),
                enabled: r.enabled,
                freeswitch_extension_id: r.freeswitch_extension_id,
                sync_status: r.sync_status,
                sync_error: r.sync_error,
                created_at: r.created_at,
                updated_at: r.updated_at,
            };
            Ok(HttpResponse::Ok().json(ApiResponse::success(route)))
        }
        None => Err(AppError::NotFound(format!("Inbound route {} not found", route_id))),
    }
}

/// Create an inbound route
///
/// POST /api/v1/routing/inbound
#[instrument(skip(pool, admin, req))]
pub async fn create_inbound_route(
    pool: web::Data<PgPool>,
    req: web::Json<CreateInboundRouteRequest>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!(name = %req.name, did = %req.did_pattern, "Creating inbound route");

    if req.name.trim().is_empty() {
        return Err(AppError::Validation("Route name cannot be empty".to_string()));
    }
    if req.destination_host.trim().is_empty() {
        return Err(AppError::Validation("Destination host cannot be empty".to_string()));
    }

    let failover_json = serde_json::to_value(&req.failover_destinations).unwrap_or_default();
    let extension_id = slugify(&req.name);

    // Get trunk name if trunk_id is provided
    let trunk_name: Option<String> = if let Some(trunk_id) = &req.destination_trunk_id {
        sqlx::query_scalar!("SELECT name FROM routing_trunks WHERE id = $1", trunk_id)
            .fetch_optional(pool.get_ref())
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    let r = sqlx::query!(
        r#"
        INSERT INTO routing_inbound_routes (name, description, did_pattern, source_ip_pattern,
                                           priority, destination_host, destination_port,
                                           destination_profile, call_timeout, inherit_codec,
                                           ignore_early_media, bypass_media, strip_digits,
                                           prefix_to_add, destination_trunk_id, send_early_media,
                                           failover_destinations, enabled, freeswitch_extension_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
        RETURNING id, name, description, did_pattern, source_ip_pattern, priority,
                  destination_host, destination_port, destination_profile,
                  call_timeout, inherit_codec, ignore_early_media, bypass_media,
                  strip_digits, prefix_to_add, destination_trunk_id, send_early_media,
                  failover_destinations, enabled, freeswitch_extension_id,
                  sync_status, sync_error, created_at, updated_at
        "#,
        req.name.trim(),
        req.description,
        req.did_pattern.trim(),
        req.source_ip_pattern,
        req.priority,
        req.destination_host.trim(),
        req.destination_port,
        req.destination_profile,
        req.call_timeout,
        req.inherit_codec,
        req.ignore_early_media,
        req.bypass_media,
        req.strip_digits,
        req.prefix_to_add.trim(),
        req.destination_trunk_id,
        req.send_early_media,
        failover_json,
        req.enabled,
        extension_id
    )
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let route = InboundRoute {
        id: r.id,
        name: r.name,
        description: r.description,
        did_pattern: r.did_pattern,
        source_ip_pattern: r.source_ip_pattern,
        priority: r.priority,
        destination_host: r.destination_host,
        destination_port: r.destination_port,
        destination_profile: r.destination_profile,
        call_timeout: r.call_timeout,
        inherit_codec: r.inherit_codec,
        ignore_early_media: r.ignore_early_media,
        bypass_media: r.bypass_media,
        strip_digits: r.strip_digits,
        prefix_to_add: r.prefix_to_add,
        destination_trunk_id: r.destination_trunk_id,
        destination_trunk_name: trunk_name,
        send_early_media: Some(r.send_early_media),
        failover_destinations: serde_json::from_value(r.failover_destinations.unwrap_or_default()).unwrap_or_default(),
        enabled: r.enabled,
        freeswitch_extension_id: r.freeswitch_extension_id,
        sync_status: r.sync_status,
        sync_error: r.sync_error,
        created_at: r.created_at,
        updated_at: r.updated_at,
    };

    // Sync to FreeSWITCH
    match sync_inbound_routes_to_freeswitch(&pool).await {
        Ok(_) => {
            let _ = sqlx::query!(
                "UPDATE routing_inbound_routes SET sync_status = 'synced', sync_error = NULL WHERE id = $1",
                route.id
            )
            .execute(pool.get_ref())
            .await;

            log_sync_operation(&pool, "create", "inbound_route", Some(route.id), "freeswitch", "success", None).await;
        }
        Err(e) => {
            let error_msg = e.to_string();
            let _ = sqlx::query!(
                "UPDATE routing_inbound_routes SET sync_status = 'error', sync_error = $1 WHERE id = $2",
                &error_msg,
                route.id
            )
            .execute(pool.get_ref())
            .await;

            log_sync_operation(&pool, "create", "inbound_route", Some(route.id), "freeswitch", "error", Some(&error_msg)).await;
        }
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("create_inbound_route")
        .entity_type("routing_inbound_route")
        .entity_id(route.id.to_string())
        .details(json!({ "name": route.name, "did": route.did_pattern }))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Created().json(ApiResponse::success(route)))
}

/// Update an inbound route
///
/// PUT /api/v1/routing/inbound/{id}
#[instrument(skip(pool, admin, req))]
pub async fn update_inbound_route(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    req: web::Json<UpdateInboundRouteRequest>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let route_id = path.into_inner();
    info!(route_id = %route_id, "Updating inbound route");

    let failover_json = req.failover_destinations.as_ref().map(|f| serde_json::to_value(f).unwrap_or_default());

    // Note: For destination_trunk_id, we use a direct assignment to allow setting it to NULL
    let r = sqlx::query!(
        r#"
        UPDATE routing_inbound_routes SET
            name = COALESCE($1, name),
            description = COALESCE($2, description),
            did_pattern = COALESCE($3, did_pattern),
            source_ip_pattern = COALESCE($4, source_ip_pattern),
            priority = COALESCE($5, priority),
            destination_host = COALESCE($6, destination_host),
            destination_port = COALESCE($7, destination_port),
            destination_profile = COALESCE($8, destination_profile),
            call_timeout = COALESCE($9, call_timeout),
            inherit_codec = COALESCE($10, inherit_codec),
            ignore_early_media = COALESCE($11, ignore_early_media),
            bypass_media = COALESCE($12, bypass_media),
            strip_digits = COALESCE($13, strip_digits),
            prefix_to_add = COALESCE($14, prefix_to_add),
            destination_trunk_id = $15,
            send_early_media = COALESCE($16, send_early_media),
            failover_destinations = COALESCE($17, failover_destinations),
            enabled = COALESCE($18, enabled),
            sync_status = 'pending'
        WHERE id = $19
        RETURNING id, name, description, did_pattern, source_ip_pattern, priority,
                  destination_host, destination_port, destination_profile,
                  call_timeout, inherit_codec, ignore_early_media, bypass_media,
                  strip_digits, prefix_to_add, destination_trunk_id, send_early_media,
                  failover_destinations, enabled, freeswitch_extension_id,
                  sync_status, sync_error, created_at, updated_at
        "#,
        req.name.as_deref(),
        req.description,
        req.did_pattern.as_deref(),
        req.source_ip_pattern,
        req.priority,
        req.destination_host.as_deref(),
        req.destination_port,
        req.destination_profile.as_deref(),
        req.call_timeout,
        req.inherit_codec,
        req.ignore_early_media,
        req.bypass_media,
        req.strip_digits,
        req.prefix_to_add.as_deref(),
        req.destination_trunk_id,
        req.send_early_media,
        failover_json,
        req.enabled,
        route_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let r = r.ok_or_else(|| AppError::NotFound(format!("Inbound route {} not found", route_id)))?;

    // Get trunk name if trunk_id is set
    let trunk_name: Option<String> = if let Some(trunk_id) = &r.destination_trunk_id {
        sqlx::query_scalar!("SELECT name FROM routing_trunks WHERE id = $1", trunk_id)
            .fetch_optional(pool.get_ref())
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    let route = InboundRoute {
        id: r.id,
        name: r.name,
        description: r.description,
        did_pattern: r.did_pattern,
        source_ip_pattern: r.source_ip_pattern,
        priority: r.priority,
        destination_host: r.destination_host,
        destination_port: r.destination_port,
        destination_profile: r.destination_profile,
        call_timeout: r.call_timeout,
        inherit_codec: r.inherit_codec,
        ignore_early_media: r.ignore_early_media,
        bypass_media: r.bypass_media,
        strip_digits: r.strip_digits,
        prefix_to_add: r.prefix_to_add,
        destination_trunk_id: r.destination_trunk_id,
        destination_trunk_name: trunk_name,
        send_early_media: Some(r.send_early_media),
        failover_destinations: serde_json::from_value(r.failover_destinations.unwrap_or_default()).unwrap_or_default(),
        enabled: r.enabled,
        freeswitch_extension_id: r.freeswitch_extension_id,
        sync_status: r.sync_status,
        sync_error: r.sync_error,
        created_at: r.created_at,
        updated_at: r.updated_at,
    };

    // Sync to FreeSWITCH
    match sync_inbound_routes_to_freeswitch(&pool).await {
        Ok(_) => {
            let _ = sqlx::query!(
                "UPDATE routing_inbound_routes SET sync_status = 'synced', sync_error = NULL WHERE id = $1",
                route.id
            )
            .execute(pool.get_ref())
            .await;

            log_sync_operation(&pool, "update", "inbound_route", Some(route.id), "freeswitch", "success", None).await;
        }
        Err(e) => {
            let error_msg = e.to_string();
            let _ = sqlx::query!(
                "UPDATE routing_inbound_routes SET sync_status = 'error', sync_error = $1 WHERE id = $2",
                &error_msg,
                route.id
            )
            .execute(pool.get_ref())
            .await;

            log_sync_operation(&pool, "update", "inbound_route", Some(route.id), "freeswitch", "error", Some(&error_msg)).await;
        }
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("update_inbound_route")
        .entity_type("routing_inbound_route")
        .entity_id(route.id.to_string())
        .details(json!({ "name": route.name }))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(route)))
}

/// Delete an inbound route
///
/// DELETE /api/v1/routing/inbound/{id}
#[instrument(skip(pool, admin))]
pub async fn delete_inbound_route(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let route_id = path.into_inner();
    info!(route_id = %route_id, "Deleting inbound route");

    let result = sqlx::query!("DELETE FROM routing_inbound_routes WHERE id = $1", route_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Inbound route {} not found", route_id)));
    }

    // Regenerate FreeSWITCH XML
    match sync_inbound_routes_to_freeswitch(&pool).await {
        Ok(_) => {
            log_sync_operation(&pool, "delete", "inbound_route", Some(route_id), "freeswitch", "success", None).await;
        }
        Err(e) => {
            log_sync_operation(&pool, "delete", "inbound_route", Some(route_id), "freeswitch", "error", Some(&e.to_string())).await;
        }
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("delete_inbound_route")
        .entity_type("routing_inbound_route")
        .entity_id(route_id.to_string())
        .details(json!({}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        json!({ "deleted": route_id }),
        "Inbound route deleted successfully",
    )))
}

// ============================================================================
// System Handlers (Reload, Sync Status, Migration)
// ============================================================================

/// Reload both Kamailio and FreeSWITCH
///
/// POST /api/v1/routing/reload
#[instrument(skip(pool, _admin))]
pub async fn reload_routing(
    pool: web::Data<PgPool>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!("Reloading routing configuration");

    let mut result = ReloadResult {
        kamailio_reloaded: false,
        freeswitch_reloaded: false,
        kamailio_message: None,
        freeswitch_message: None,
    };

    // Reload Kamailio
    match reload_kamailio().await {
        Ok(msg) => {
            result.kamailio_reloaded = true;
            result.kamailio_message = Some(msg);
            log_sync_operation(&pool, "reload", "system", None, "kamailio", "success", None).await;
        }
        Err(e) => {
            result.kamailio_message = Some(e.to_string());
            log_sync_operation(&pool, "reload", "system", None, "kamailio", "error", Some(&e.to_string())).await;
        }
    }

    // Reload FreeSWITCH
    match reload_freeswitch().await {
        Ok(msg) => {
            result.freeswitch_reloaded = true;
            result.freeswitch_message = Some(msg);
            log_sync_operation(&pool, "reload", "system", None, "freeswitch", "success", None).await;
        }
        Err(e) => {
            result.freeswitch_message = Some(e.to_string());
            log_sync_operation(&pool, "reload", "system", None, "freeswitch", "error", Some(&e.to_string())).await;
        }
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

/// Get synchronization status
///
/// GET /api/v1/routing/sync-status
#[instrument(skip(pool, _admin))]
pub async fn get_sync_status(
    pool: web::Data<PgPool>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!("Getting sync status");

    let status = sqlx::query!(
        r#"
        SELECT
            (SELECT COUNT(*) FROM routing_trunks)::bigint as trunks_total,
            (SELECT COUNT(*) FROM routing_trunks WHERE sync_status = 'synced')::bigint as trunks_synced,
            (SELECT COUNT(*) FROM routing_trunks WHERE sync_status = 'pending')::bigint as trunks_pending,
            (SELECT COUNT(*) FROM routing_trunks WHERE sync_status = 'error')::bigint as trunks_error,
            (SELECT COUNT(*) FROM routing_trunk_groups)::bigint as trunk_groups_total,
            (SELECT COUNT(*) FROM routing_trunk_groups WHERE sync_status = 'synced')::bigint as trunk_groups_synced,
            (SELECT COUNT(*) FROM routing_outbound_routes)::bigint as outbound_routes_total,
            (SELECT COUNT(*) FROM routing_outbound_routes WHERE sync_status = 'synced')::bigint as outbound_routes_synced,
            (SELECT COUNT(*) FROM routing_inbound_routes)::bigint as inbound_routes_total,
            (SELECT COUNT(*) FROM routing_inbound_routes WHERE sync_status = 'synced')::bigint as inbound_routes_synced,
            (SELECT created_at FROM routing_sync_log WHERE status = 'success' ORDER BY created_at DESC LIMIT 1) as last_sync
        "#
    )
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let result = RoutingSyncStatus {
        trunks_total: status.trunks_total.unwrap_or(0),
        trunks_synced: status.trunks_synced.unwrap_or(0),
        trunks_pending: status.trunks_pending.unwrap_or(0),
        trunks_error: status.trunks_error.unwrap_or(0),
        trunk_groups_total: status.trunk_groups_total.unwrap_or(0),
        trunk_groups_synced: status.trunk_groups_synced.unwrap_or(0),
        outbound_routes_total: status.outbound_routes_total.unwrap_or(0),
        outbound_routes_synced: status.outbound_routes_synced.unwrap_or(0),
        inbound_routes_total: status.inbound_routes_total.unwrap_or(0),
        inbound_routes_synced: status.inbound_routes_synced.unwrap_or(0),
        kamailio_connected: true, // TODO: Actually check connection
        freeswitch_connected: true, // TODO: Actually check connection
        last_sync: status.last_sync,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

/// Migrate existing routes from Kamailio and FreeSWITCH
///
/// POST /api/v1/routing/migrate
#[instrument(skip(pool, mysql_pool, admin))]
pub async fn migrate_routes(
    pool: web::Data<PgPool>,
    mysql_pool: Option<web::Data<MySqlPool>>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!("Starting route migration");

    let mut result = MigrationResult {
        trunks_imported: 0,
        trunk_groups_imported: 0,
        outbound_routes_imported: 0,
        inbound_routes_imported: 0,
        warnings: Vec::new(),
        errors: Vec::new(),
    };

    // Migrate from Kamailio
    if let Some(mysql) = mysql_pool {
        // Import trunks from dr_gateways
        match import_trunks_from_kamailio(&pool, &mysql).await {
            Ok(count) => result.trunks_imported = count,
            Err(e) => result.errors.push(format!("Trunk import failed: {}", e)),
        }

        // Import groups from dr_gw_lists
        match import_trunk_groups_from_kamailio(&pool, &mysql).await {
            Ok(count) => result.trunk_groups_imported = count,
            Err(e) => result.errors.push(format!("Trunk group import failed: {}", e)),
        }

        // Import routes from dr_rules
        match import_outbound_routes_from_kamailio(&pool, &mysql).await {
            Ok(count) => result.outbound_routes_imported = count,
            Err(e) => result.errors.push(format!("Outbound route import failed: {}", e)),
        }
    } else {
        result.warnings.push("Kamailio MySQL not configured - skipping Kamailio migration".to_string());
    }

    // Import inbound routes from FreeSWITCH XML
    match import_inbound_routes_from_freeswitch(&pool).await {
        Ok(count) => result.inbound_routes_imported = count,
        Err(e) => result.errors.push(format!("Inbound route import failed: {}", e)),
    }

    log_sync_operation(&pool, "migrate", "system", None, "both", "success", Some(&format!("{:?}", result))).await;

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("migrate_routes")
        .entity_type("routing")
        .entity_id("migration".to_string())
        .details(json!(result))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

/// Sync all pending/error routes from PostgreSQL to Kamailio
///
/// POST /api/v1/routing/sync-to-kamailio
#[instrument(skip(pool, mysql_pool, admin))]
pub async fn sync_to_kamailio(
    pool: web::Data<PgPool>,
    mysql_pool: Option<web::Data<MySqlPool>>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!("Syncing routes to Kamailio");

    let mysql = mysql_pool.ok_or_else(|| {
        AppError::Internal("Kamailio MySQL not configured".to_string())
    })?;

    let mut result = SyncToKamailioResult {
        trunks_synced: 0,
        trunks_failed: 0,
        trunk_groups_synced: 0,
        trunk_groups_failed: 0,
        outbound_routes_synced: 0,
        outbound_routes_failed: 0,
        kamailio_reloaded: false,
        errors: Vec::new(),
    };

    // Step 1: Sync trunks that are pending, error, or not synced
    let trunks = sqlx::query_as!(
        Trunk,
        r#"
        SELECT id, name, description, host, port, transport,
               auth_username, auth_password_encrypted, auth_password_nonce, strip_digits,
               prefix_to_add, enabled, trunk_type, freeswitch_gateway_name,
               kamailio_gwid, sync_status, sync_error,
               sip_status, sip_status_message, last_options_check,
               last_options_latency_ms, last_options_response_code,
               created_at, updated_at
        FROM routing_trunks
        WHERE trunk_type = 'public' AND (sync_status != 'synced' OR kamailio_gwid IS NULL)
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    for trunk in trunks {
        match sync_trunk_to_kamailio(&pool, &mysql, &trunk).await {
            Ok(gwid) => {
                let _ = sqlx::query!(
                    "UPDATE routing_trunks SET kamailio_gwid = $1, sync_status = 'synced', sync_error = NULL WHERE id = $2",
                    gwid,
                    trunk.id
                )
                .execute(pool.get_ref())
                .await;
                result.trunks_synced += 1;
            }
            Err(e) => {
                let error_msg = format!("Trunk {}: {}", trunk.name, e);
                let _ = sqlx::query!(
                    "UPDATE routing_trunks SET sync_status = 'error', sync_error = $1 WHERE id = $2",
                    &error_msg,
                    trunk.id
                )
                .execute(pool.get_ref())
                .await;
                result.trunks_failed += 1;
                result.errors.push(error_msg);
            }
        }
    }

    // Step 2: Sync trunk groups that are pending, error, or not synced
    let groups = sqlx::query_as!(
        TrunkGroup,
        r#"
        SELECT id, name, description, failover_strategy,
               kamailio_group_id, sync_status, sync_error,
               created_at, updated_at
        FROM routing_trunk_groups
        WHERE sync_status != 'synced' OR kamailio_group_id IS NULL
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    for group in groups {
        // Get trunk IDs for this group
        let trunk_ids: Vec<Uuid> = sqlx::query_scalar!(
            "SELECT trunk_id FROM routing_trunk_group_members WHERE group_id = $1 ORDER BY priority",
            group.id
        )
        .fetch_all(pool.get_ref())
        .await
        .unwrap_or_default();

        match sync_trunk_group_to_kamailio(&pool, &mysql, &group, &trunk_ids).await {
            Ok(group_id) => {
                let _ = sqlx::query!(
                    "UPDATE routing_trunk_groups SET kamailio_group_id = $1, sync_status = 'synced', sync_error = NULL WHERE id = $2",
                    group_id,
                    group.id
                )
                .execute(pool.get_ref())
                .await;
                result.trunk_groups_synced += 1;
            }
            Err(e) => {
                let error_msg = format!("Trunk group {}: {}", group.name, e);
                let _ = sqlx::query!(
                    "UPDATE routing_trunk_groups SET sync_status = 'error', sync_error = $1 WHERE id = $2",
                    &error_msg,
                    group.id
                )
                .execute(pool.get_ref())
                .await;
                result.trunk_groups_failed += 1;
                result.errors.push(error_msg);
            }
        }
    }

    // Step 3: Sync outbound routes that are pending, error, or not synced
    let routes = sqlx::query_as!(
        OutboundRoute,
        r#"
        SELECT id, name, description, prefix_pattern, priority,
               trunk_group_id, NULL as trunk_group_name,
               trunk_id, NULL as trunk_name,
               time_schedule, time_schedule_enabled, enabled,
               kamailio_ruleid, sync_status, sync_error,
               created_at, updated_at
        FROM routing_outbound_routes
        WHERE sync_status != 'synced' OR kamailio_ruleid IS NULL
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    for route in routes {
        match sync_outbound_route_to_kamailio(&pool, &mysql, &route).await {
            Ok(ruleid) => {
                let _ = sqlx::query!(
                    "UPDATE routing_outbound_routes SET kamailio_ruleid = $1, sync_status = 'synced', sync_error = NULL WHERE id = $2",
                    ruleid,
                    route.id
                )
                .execute(pool.get_ref())
                .await;
                result.outbound_routes_synced += 1;
            }
            Err(e) => {
                let error_msg = format!("Route {}: {}", route.name, e);
                let _ = sqlx::query!(
                    "UPDATE routing_outbound_routes SET sync_status = 'error', sync_error = $1 WHERE id = $2",
                    &error_msg,
                    route.id
                )
                .execute(pool.get_ref())
                .await;
                result.outbound_routes_failed += 1;
                result.errors.push(error_msg);
            }
        }
    }

    // Step 4: Reload Kamailio drouting
    match reload_kamailio().await {
        Ok(_) => {
            result.kamailio_reloaded = true;
        }
        Err(e) => {
            result.errors.push(format!("Kamailio reload failed: {}", e));
        }
    }

    log_sync_operation(&pool, "sync_to_kamailio", "system", None, "kamailio", "success", Some(&format!("{:?}", result))).await;

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("sync_to_kamailio")
        .entity_type("routing")
        .entity_id("sync".to_string())
        .details(json!(result))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

/// Force re-sync all routes to Kamailio (deletes and re-creates)
///
/// POST /api/v1/routing/force-sync-kamailio
#[instrument(skip(pool, mysql_pool, admin))]
pub async fn force_sync_to_kamailio(
    pool: web::Data<PgPool>,
    mysql_pool: Option<web::Data<MySqlPool>>,
    admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!("Force syncing all routes to Kamailio");

    let mysql = mysql_pool.ok_or_else(|| {
        AppError::Internal("Kamailio MySQL not configured".to_string())
    })?;

    let mut result = SyncToKamailioResult {
        trunks_synced: 0,
        trunks_failed: 0,
        trunk_groups_synced: 0,
        trunk_groups_failed: 0,
        outbound_routes_synced: 0,
        outbound_routes_failed: 0,
        kamailio_reloaded: false,
        errors: Vec::new(),
    };

    // Delete all existing outbound routes from dr_rules with groupid 8000
    let _ = sqlx::query("DELETE FROM dr_rules WHERE groupid = '8000'")
        .execute(mysql.get_ref())
        .await;

    // Reset sync status for all outbound routes
    let _ = sqlx::query!(
        "UPDATE routing_outbound_routes SET kamailio_ruleid = NULL, sync_status = 'pending', sync_error = NULL"
    )
    .execute(pool.get_ref())
    .await;

    // Get all enabled outbound routes
    let routes = sqlx::query_as!(
        OutboundRoute,
        r#"
        SELECT id, name, description, prefix_pattern, priority,
               trunk_group_id, NULL as trunk_group_name,
               trunk_id, NULL as trunk_name,
               time_schedule, time_schedule_enabled, enabled,
               kamailio_ruleid, sync_status, sync_error,
               created_at, updated_at
        FROM routing_outbound_routes
        WHERE enabled = true
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    for route in routes {
        match sync_outbound_route_to_kamailio(&pool, &mysql, &route).await {
            Ok(ruleid) => {
                let _ = sqlx::query!(
                    "UPDATE routing_outbound_routes SET kamailio_ruleid = $1, sync_status = 'synced', sync_error = NULL WHERE id = $2",
                    ruleid,
                    route.id
                )
                .execute(pool.get_ref())
                .await;
                result.outbound_routes_synced += 1;
            }
            Err(e) => {
                let error_msg = format!("Route {}: {}", route.name, e);
                let _ = sqlx::query!(
                    "UPDATE routing_outbound_routes SET sync_status = 'error', sync_error = $1 WHERE id = $2",
                    &error_msg,
                    route.id
                )
                .execute(pool.get_ref())
                .await;
                result.outbound_routes_failed += 1;
                result.errors.push(error_msg);
            }
        }
    }

    // Reload Kamailio drouting
    match reload_kamailio().await {
        Ok(_) => {
            result.kamailio_reloaded = true;
        }
        Err(e) => {
            result.errors.push(format!("Kamailio reload failed: {}", e));
        }
    }

    log_sync_operation(&pool, "force_sync_to_kamailio", "system", None, "kamailio", "success", Some(&format!("{:?}", result))).await;

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("force_sync_to_kamailio")
        .entity_type("routing")
        .entity_id("force_sync".to_string())
        .details(json!(result))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

// ============================================================================
// Route Configuration
// ============================================================================

/// Configure unified routing routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/routing")
            // Trunks
            .route("/trunks", web::get().to(list_trunks))
            .route("/trunks", web::post().to(create_trunk))
            .route("/trunks/{id}", web::get().to(get_trunk))
            .route("/trunks/{id}", web::put().to(update_trunk))
            .route("/trunks/{id}", web::delete().to(delete_trunk))
            // Trunk Groups
            .route("/trunk-groups", web::get().to(list_trunk_groups))
            .route("/trunk-groups", web::post().to(create_trunk_group))
            .route("/trunk-groups/{id}", web::get().to(get_trunk_group))
            .route("/trunk-groups/{id}", web::put().to(update_trunk_group))
            .route("/trunk-groups/{id}", web::delete().to(delete_trunk_group))
            // Outbound Routes
            .route("/outbound", web::get().to(list_outbound_routes))
            .route("/outbound", web::post().to(create_outbound_route))
            .route("/outbound/{id}", web::get().to(get_outbound_route))
            .route("/outbound/{id}", web::put().to(update_outbound_route))
            .route("/outbound/{id}", web::delete().to(delete_outbound_route))
            // Inbound Routes
            .route("/inbound", web::get().to(list_inbound_routes))
            .route("/inbound", web::post().to(create_inbound_route))
            .route("/inbound/{id}", web::get().to(get_inbound_route))
            .route("/inbound/{id}", web::put().to(update_inbound_route))
            .route("/inbound/{id}", web::delete().to(delete_inbound_route))
            // System
            .route("/reload", web::post().to(reload_routing))
            .route("/sync-status", web::get().to(get_sync_status))
            .route("/migrate", web::post().to(migrate_routes))
            .route("/sync-to-kamailio", web::post().to(sync_to_kamailio))
            .route("/force-sync-kamailio", web::post().to(force_sync_to_kamailio))
            // SIP Status
            .route("/sip-status", web::get().to(check_all_trunks_sip_status))
            .route("/sip-status/{id}", web::get().to(check_trunk_sip_status))
            .route("/sip-status/{id}/history", web::get().to(get_trunk_sip_history)),
    );
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert name to slug
fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
        .join("-")
}

/// Log a sync operation
async fn log_sync_operation(
    pool: &PgPool,
    operation: &str,
    entity_type: &str,
    entity_id: Option<Uuid>,
    target_system: &str,
    status: &str,
    error_message: Option<&str>,
) {
    let _ = sqlx::query!(
        r#"
        INSERT INTO routing_sync_log (operation, entity_type, entity_id, target_system, status, error_message)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        operation,
        entity_type,
        entity_id,
        target_system,
        status,
        error_message
    )
    .execute(pool)
    .await;
}

// ============================================================================
// Kamailio Sync Functions
// ============================================================================

/// Sync a trunk to Kamailio dr_gateways
async fn sync_trunk_to_kamailio(
    _pg_pool: &PgPool,
    mysql_pool: &MySqlPool,
    trunk: &Trunk,
) -> Result<i32, AppError> {
    let port = trunk.port.unwrap_or(5060);
    let strip_digits = trunk.strip_digits.unwrap_or(0) as u32;
    let prefix = trunk.prefix_to_add.as_deref().unwrap_or("");
    let address = format!("{}:{}", trunk.host, port);

    let result = sqlx::query(
        "INSERT INTO dr_gateways (type, address, strip, pri_prefix, attrs, description) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(1i32) // type 1 = SIP gateway
    .bind(&address)
    .bind(strip_digits)
    .bind(prefix)
    .bind("")
    .bind(&trunk.name)
    .execute(mysql_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to sync trunk to Kamailio: {}", e)))?;

    // Reload Kamailio drouting
    reload_kamailio_drouting().await;

    Ok(result.last_insert_id() as i32)
}

/// Update a trunk in Kamailio
async fn update_trunk_in_kamailio(
    mysql_pool: &MySqlPool,
    gwid: i32,
    trunk: &Trunk,
) -> Result<(), AppError> {
    let port = trunk.port.unwrap_or(5060);
    let strip_digits = trunk.strip_digits.unwrap_or(0) as u32;
    let prefix = trunk.prefix_to_add.as_deref().unwrap_or("");
    let address = format!("{}:{}", trunk.host, port);

    sqlx::query(
        "UPDATE dr_gateways SET address = ?, strip = ?, pri_prefix = ?, description = ? WHERE gwid = ?"
    )
    .bind(&address)
    .bind(strip_digits)
    .bind(prefix)
    .bind(&trunk.name)
    .bind(gwid as u32)
    .execute(mysql_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to update trunk in Kamailio: {}", e)))?;

    // Reload Kamailio drouting
    reload_kamailio_drouting().await;

    Ok(())
}

/// Reload Kamailio drouting module to apply changes
async fn reload_kamailio_drouting() {
    use tokio::process::Command;

    // Try kamcmd first (preferred)
    let result = Command::new("kamcmd")
        .args(["drouting.reload"])
        .output()
        .await;

    match result {
        Ok(output) if output.status.success() => {
            info!("Kamailio drouting reloaded via kamcmd");
        }
        _ => {
            // Fallback to kamctl
            let fallback = Command::new("kamctl")
                .args(["rpc", "drouting.reload"])
                .output()
                .await;

            match fallback {
                Ok(output) if output.status.success() => {
                    info!("Kamailio drouting reloaded via kamctl");
                }
                _ => {
                    warn!("Could not reload Kamailio drouting - manual reload may be required");
                }
            }
        }
    }
}

/// Delete a trunk from Kamailio
async fn delete_trunk_from_kamailio(mysql_pool: &MySqlPool, gwid: i32) -> Result<(), AppError> {
    // First, remove the gwid from any dr_gw_lists that reference it
    let gwid_str = gwid.to_string();

    // Get all groups that contain this gwid
    let groups: Vec<(i32, String)> = sqlx::query_as("SELECT id, gwlist FROM dr_gw_lists WHERE gwlist LIKE ?")
        .bind(format!("%{}%", gwid))
        .fetch_all(mysql_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to query dr_gw_lists: {}", e)))?;

    // Update each group to remove the gwid
    for (group_id, gwlist) in groups {
        let new_gwlist: String = gwlist
            .split(',')
            .filter(|g| g.trim() != gwid_str)
            .collect::<Vec<_>>()
            .join(",");

        if new_gwlist.is_empty() {
            // If no gateways left, delete the group
            sqlx::query("DELETE FROM dr_gw_lists WHERE id = ?")
                .bind(group_id as u32)
                .execute(mysql_pool)
                .await
                .map_err(|e| AppError::Database(format!("Failed to delete empty group: {}", e)))?;
            info!(group_id = group_id, "Deleted empty Kamailio group after removing gateway");
        } else {
            // Update the group with remaining gateways
            sqlx::query("UPDATE dr_gw_lists SET gwlist = ? WHERE id = ?")
                .bind(&new_gwlist)
                .bind(group_id as u32)
                .execute(mysql_pool)
                .await
                .map_err(|e| AppError::Database(format!("Failed to update dr_gw_lists: {}", e)))?;
            info!(group_id = group_id, old_gwlist = %gwlist, new_gwlist = %new_gwlist, "Updated Kamailio group after removing gateway");
        }
    }

    // Now delete the gateway itself
    sqlx::query("DELETE FROM dr_gateways WHERE gwid = ?")
        .bind(gwid as u32)
        .execute(mysql_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete trunk from Kamailio: {}", e)))?;

    // Reload Kamailio drouting
    reload_kamailio_drouting().await;

    Ok(())
}

// ============================================================================
// FreeSWITCH Sync Functions
// ============================================================================

const FS_GATEWAYS_DIR: &str = "/etc/freeswitch/sip_profiles/external/gateways";

/// Sync a trunk to FreeSWITCH as a gateway
async fn sync_trunk_to_freeswitch(
    _pool: &PgPool,
    trunk: &Trunk,
    gateway_name: &str,
    sip_service: Option<&Arc<SipDeviceService>>,
) -> Result<(), AppError> {
    let port = trunk.port.unwrap_or(5060);
    let transport = trunk.transport.as_deref().unwrap_or("udp");
    let username = trunk.auth_username.as_deref().unwrap_or("");

    // Decrypt password if available
    let password = if let (Some(encrypted), Some(nonce)) = (&trunk.auth_password_encrypted, &trunk.auth_password_nonce) {
        if let Some(sip_svc) = sip_service {
            match sip_svc.decrypt_password(encrypted, nonce) {
                Ok(pwd) => Some(pwd),
                Err(e) => {
                    warn!(error = %e, "Failed to decrypt trunk password");
                    None
                }
            }
        } else {
            warn!("SIP service not available, cannot decrypt password");
            None
        }
    } else {
        None
    };

    // Build password parameter line if password is available
    let password_line = password.as_ref()
        .map(|pwd| format!("    <param name=\"password\" value=\"{}\"/>\n", pwd))
        .unwrap_or_default();

    let xml_content = format!(
        r#"<include>
  <!-- Gateway: {} - Auto-generated by Apolo SBC -->
  <gateway name="{}">
    <param name="realm" value="{}"/>
    <param name="proxy" value="{}:{}"/>
    <param name="register" value="{}"/>
    <param name="username" value="{}"/>
{}    <param name="register-transport" value="{}"/>
    <param name="ping" value="25"/>
    <param name="ping-max" value="3"/>
    <param name="ping-min" value="1"/>
    <param name="retry-seconds" value="30"/>
  </gateway>
</include>
"#,
        trunk.name,
        gateway_name,
        trunk.host,
        trunk.host,
        port,
        if username.is_empty() && password.is_none() { "false" } else { "true" },
        username,
        password_line,
        transport
    );

    // Ensure directory exists
    let gateway_dir = Path::new(FS_GATEWAYS_DIR);
    if !gateway_dir.exists() {
        fs::create_dir_all(gateway_dir)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create gateways directory: {}", e)))?;
    }

    // Write gateway XML file
    let gateway_path = gateway_dir.join(format!("{}.xml", gateway_name));
    fs::write(&gateway_path, xml_content)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to write gateway file: {}", e)))?;

    info!(gateway = %gateway_name, path = ?gateway_path, "Created FreeSWITCH gateway XML");

    // Reload gateway in FreeSWITCH
    reload_freeswitch_gateway(gateway_name).await?;

    Ok(())
}

/// Reload a gateway in FreeSWITCH (kill existing + rescan profile)
async fn reload_freeswitch_gateway(gateway_name: &str) -> Result<(), AppError> {
    use tokio::process::Command;

    // Kill existing gateway if it exists
    let kill_result = Command::new("fs_cli")
        .args(["-x", &format!("sofia profile external killgw {}", gateway_name)])
        .output()
        .await;

    if let Ok(output) = kill_result {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("marked for deletion") {
            info!(gateway = %gateway_name, "Old gateway marked for deletion");
            // Wait a moment for cleanup
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }

    // Rescan profile to load new/updated gateway
    let rescan_result = Command::new("fs_cli")
        .args(["-x", "sofia profile external rescan reloadxml"])
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to execute fs_cli rescan: {}", e)))?;

    let stdout = String::from_utf8_lossy(&rescan_result.stdout);
    if rescan_result.status.success() && stdout.contains("scan complete") {
        info!(gateway = %gateway_name, "FreeSWITCH gateway reloaded successfully");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&rescan_result.stderr);
        warn!(gateway = %gateway_name, stdout = %stdout, stderr = %stderr, "FreeSWITCH rescan completed with warnings");
        // Don't fail - the gateway might still work
        Ok(())
    }
}

/// Delete a trunk from FreeSWITCH
async fn delete_trunk_from_freeswitch(gateway_name: &str) -> Result<(), AppError> {
    use tokio::process::Command;

    // Kill gateway in FreeSWITCH first
    let _ = Command::new("fs_cli")
        .args(["-x", &format!("sofia profile external killgw {}", gateway_name)])
        .output()
        .await;

    // Remove XML file
    let gateway_path = Path::new(FS_GATEWAYS_DIR).join(format!("{}.xml", gateway_name));
    if gateway_path.exists() {
        fs::remove_file(&gateway_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to delete gateway file: {}", e)))?;
    }

    // Rescan to apply changes
    let _ = Command::new("fs_cli")
        .args(["-x", "sofia profile external rescan reloadxml"])
        .output()
        .await;

    info!(gateway = %gateway_name, "Deleted FreeSWITCH gateway");
    Ok(())
}

// ============================================================================
// SIP Status Check Handlers
// ============================================================================

/// Check SIP status for all trunks
///
/// GET /api/v1/routing/sip-status
#[instrument(skip(pool, _admin))]
pub async fn check_all_trunks_sip_status(
    pool: web::Data<PgPool>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!("Checking SIP status for all trunks");

    let trunks = sqlx::query!(
        r#"
        SELECT id, name, trunk_type, host, port, freeswitch_gateway_name, sip_status, last_options_check
        FROM routing_trunks
        WHERE enabled = true
        ORDER BY name
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let mut results: Vec<SipStatusCheck> = Vec::new();
    let mut reachable = 0;
    let mut unreachable = 0;
    let mut errors = 0;

    for trunk in trunks {
        let trunk_type = trunk.trunk_type.as_deref().unwrap_or("public");
        let port = trunk.port.unwrap_or(5060);
        let gateway_name_str = trunk.freeswitch_gateway_name.clone().unwrap_or_else(|| trunk.name.clone());
        let gateway_name = gateway_name_str.as_str();

        // Perform OPTIONS check based on trunk type
        let check_result = if trunk_type == "private" {
            check_sip_status_via_freeswitch(gateway_name, &trunk.host, port).await
        } else {
            check_sip_status_via_kamailio(&trunk.host, port).await
        };

        let (status, status_message, response_code, latency_ms, sip_exchange) = match check_result {
            Ok(result) => {
                // Count based on actual status, not just Ok/Err
                if result.0 == "reachable" {
                    reachable += 1;
                } else {
                    unreachable += 1;
                }
                result
            }
            Err(e) => {
                errors += 1;
                ("error".to_string(), e.to_string(), None, None, None)
            }
        };

        // Update trunk status in database
        let now = chrono::Utc::now();
        let _ = sqlx::query!(
            r#"
            UPDATE routing_trunks
            SET sip_status = $1, sip_status_message = $2,
                last_options_check = $3, last_options_latency_ms = $4,
                last_options_response_code = $5
            WHERE id = $6
            "#,
            &status,
            &status_message,
            now,
            latency_ms,
            response_code,
            trunk.id
        )
        .execute(pool.get_ref())
        .await;

        // Log the check
        let _ = sqlx::query!(
            r#"
            INSERT INTO routing_sip_status_log
            (trunk_id, check_source, status, response_code, latency_ms, error_message)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            trunk.id,
            trunk_type,
            if status == "reachable" { "success" } else { "error" },
            response_code,
            latency_ms,
            if status == "reachable" { None } else { Some(&status_message) }
        )
        .execute(pool.get_ref())
        .await;

        results.push(SipStatusCheck {
            trunk_id: trunk.id,
            trunk_name: trunk.name,
            trunk_type: trunk_type.to_string(),
            host: trunk.host,
            port,
            gateway_name: if trunk_type == "private" { Some(gateway_name_str.clone()) } else { None },
            status,
            status_message,
            response_code,
            latency_ms,
            checked_at: now,
            check_source: trunk_type.to_string(),
            sip_exchange,
        });
    }

    let bulk_result = BulkSipStatusResult {
        total_checked: results.len() as i32,
        reachable,
        unreachable,
        errors,
        results,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(bulk_result)))
}

/// Check SIP status for a single trunk
///
/// GET /api/v1/routing/sip-status/{id}
#[instrument(skip(pool, _admin))]
pub async fn check_trunk_sip_status(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let trunk_id = path.into_inner();
    info!(trunk_id = %trunk_id, "Checking SIP status for trunk");

    let trunk = sqlx::query!(
        r#"
        SELECT id, name, trunk_type, host, port, freeswitch_gateway_name
        FROM routing_trunks
        WHERE id = $1
        "#,
        trunk_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Trunk {} not found", trunk_id)))?;

    let trunk_type = trunk.trunk_type.as_deref().unwrap_or("public");
    let port = trunk.port.unwrap_or(5060);
    let gateway_name_str = trunk.freeswitch_gateway_name.clone().unwrap_or_else(|| trunk.name.clone());
    let gateway_name = gateway_name_str.as_str();

    // Perform OPTIONS check
    let check_result = if trunk_type == "private" {
        check_sip_status_via_freeswitch(gateway_name, &trunk.host, port).await
    } else {
        check_sip_status_via_kamailio(&trunk.host, port).await
    };

    let (status, status_message, response_code, latency_ms, sip_exchange) = match check_result {
        Ok(result) => result,
        Err(e) => ("unreachable".to_string(), e.to_string(), None, None, None),
    };

    // Update trunk status
    let now = chrono::Utc::now();
    let _ = sqlx::query!(
        r#"
        UPDATE routing_trunks
        SET sip_status = $1, sip_status_message = $2,
            last_options_check = $3, last_options_latency_ms = $4,
            last_options_response_code = $5
        WHERE id = $6
        "#,
        &status,
        &status_message,
        now,
        latency_ms,
        response_code,
        trunk_id
    )
    .execute(pool.get_ref())
    .await;

    // Log the check
    let _ = sqlx::query!(
        r#"
        INSERT INTO routing_sip_status_log
        (trunk_id, check_source, status, response_code, latency_ms, error_message)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        trunk_id,
        trunk_type,
        if status == "reachable" { "success" } else { "error" },
        response_code,
        latency_ms,
        if status == "reachable" { None } else { Some(&status_message) }
    )
    .execute(pool.get_ref())
    .await;

    let result = SipStatusCheck {
        trunk_id,
        trunk_name: trunk.name,
        trunk_type: trunk_type.to_string(),
        host: trunk.host,
        port,
        gateway_name: if trunk_type == "private" { Some(gateway_name_str) } else { None },
        status,
        status_message,
        response_code,
        latency_ms,
        checked_at: now,
        check_source: trunk_type.to_string(),
        sip_exchange,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

/// Get SIP status history for a trunk
///
/// GET /api/v1/routing/sip-status/{id}/history
#[instrument(skip(pool, _admin))]
pub async fn get_trunk_sip_history(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let trunk_id = path.into_inner();
    info!(trunk_id = %trunk_id, "Getting SIP status history");

    let history = sqlx::query_as!(
        SipStatusLogEntry,
        r#"
        SELECT id, trunk_id, check_timestamp, check_source, status,
               response_code, latency_ms, error_message, peer_user_agent
        FROM routing_sip_status_log
        WHERE trunk_id = $1
        ORDER BY check_timestamp DESC
        LIMIT 100
        "#,
        trunk_id
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(history)))
}

// ============================================================================
// SIP Status Check Functions
// ============================================================================

/// Check SIP status via FreeSWITCH (for private trunks)
async fn check_sip_status_via_freeswitch(
    gateway_name: &str,
    host: &str,
    port: i32,
) -> Result<(String, String, Option<i32>, Option<i32>, Option<SipExchange>), AppError> {
    use std::time::{Duration, Instant};
    use tokio::process::Command;
    use tokio::time::timeout;

    let start = Instant::now();

    // Use fs_cli to query gateway status by gateway name with timeout
    let fs_result = timeout(
        Duration::from_secs(5),
        Command::new("fs_cli")
            .args([
                "-x",
                &format!("sofia status gateway {}", gateway_name),
            ])
            .output()
    )
    .await;

    let latency = start.elapsed().as_millis() as i32;

    // Handle timeout
    let output = match fs_result {
        Ok(result) => result,
        Err(_) => {
            return Ok((
                "unknown".to_string(),
                format!("Timeout al consultar FreeSWITCH ({}ms)", latency),
                None,
                Some(latency),
                None,
            ));
        }
    };

    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let _stderr = String::from_utf8_lossy(&result.stderr);

            if stdout.contains("REGED") || stdout.contains("UP") {
                // Build didactic SIP exchange
                let exchange = SipExchange {
                    local_endpoint: "FreeSWITCH (Apolo SBC)".to_string(),
                    remote_endpoint: format!("{}:{}", host, port),
                    request_summary: SipMessageSummary {
                        method_or_status: "OPTIONS".to_string(),
                        from: "sip:apolo-sbc@local".to_string(),
                        to: format!("sip:{}:{}", host, port),
                        call_id: Some("check-xyz123".to_string()),
                        user_agent: Some("Apolo-SBC/1.0".to_string()),
                        allow_methods: None,
                    },
                    response_summary: Some(SipMessageSummary {
                        method_or_status: "200 OK".to_string(),
                        from: format!("sip:{}:{}", host, port),
                        to: "sip:apolo-sbc@local".to_string(),
                        call_id: Some("check-xyz123".to_string()),
                        user_agent: None,
                        allow_methods: Some("INVITE, ACK, BYE, CANCEL, OPTIONS".to_string()),
                    }),
                    timeline: vec![
                        SipExchangeStep {
                            timestamp: chrono::Utc::now(),
                            direction: "outbound".to_string(),
                            message_type: "OPTIONS".to_string(),
                            description: format!("Enviando OPTIONS a {}:{}", host, port),
                            status: "success".to_string(),
                        },
                        SipExchangeStep {
                            timestamp: chrono::Utc::now(),
                            direction: "inbound".to_string(),
                            message_type: "200 OK".to_string(),
                            description: format!("Respuesta recibida en {}ms", latency),
                            status: "success".to_string(),
                        },
                    ],
                };

                Ok((
                    "reachable".to_string(),
                    format!("Gateway UP - Latencia: {}ms", latency),
                    Some(200),
                    Some(latency),
                    Some(exchange),
                ))
            } else if stdout.contains("FAIL_WAIT") || stdout.contains("TRYING") {
                // Gateway exists but registration is failing/retrying
                Ok((
                    "unreachable".to_string(),
                    format!("Registro fallido - reintentando ({}ms)", latency),
                    Some(401),
                    Some(latency),
                    None,
                ))
            } else if stdout.contains("NOREG") || stdout.contains("DOWN") {
                Ok((
                    "unreachable".to_string(),
                    format!("Gateway DOWN ({}ms)", latency),
                    Some(503),
                    Some(latency),
                    None,
                ))
            } else if stdout.contains("Invalid Gateway") {
                Ok((
                    "unknown".to_string(),
                    "Gateway no configurado en FreeSWITCH".to_string(),
                    None,
                    Some(latency),
                    None,
                ))
            } else {
                Ok((
                    "unknown".to_string(),
                    format!("Estado: {}", stdout.lines().find(|l| l.contains("State")).unwrap_or("desconocido").trim()),
                    None,
                    Some(latency),
                    None,
                ))
            }
        }
        Err(e) => {
            Err(AppError::Internal(format!("Error ejecutando fs_cli: {}", e)))
        }
    }
}

/// Check SIP status via Kamailio (for public trunks)
async fn check_sip_status_via_kamailio(
    host: &str,
    port: i32,
) -> Result<(String, String, Option<i32>, Option<i32>, Option<SipExchange>), AppError> {
    use std::time::{Duration, Instant};
    use tokio::process::Command;
    use tokio::time::timeout;
    use tokio::io::AsyncWriteExt;

    let start = Instant::now();

    // Get local IP for SIP headers from environment (required for carrier authorization)
    let local_ip = std::env::var("SIPSAK_LOCAL_IP").unwrap_or_else(|_| "127.0.0.1".to_string());

    // Generate unique Call-ID and branch
    let call_id = format!("apolo-{}-{}", std::process::id(), start.elapsed().as_nanos());
    let branch = format!("z9hG4bK-apolo-{}", &call_id[..16.min(call_id.len())]);

    // Build custom SIP OPTIONS message with ApoloSBC User-Agent
    let sip_message = format!(
        "OPTIONS sip:{}:{} SIP/2.0\r\n\
         Via: SIP/2.0/UDP {}:5060;branch={};rport\r\n\
         From: <sip:apolosbc@{}>;tag={}\r\n\
         To: <sip:{}:{}>\r\n\
         Call-ID: {}@{}\r\n\
         CSeq: 1 OPTIONS\r\n\
         Contact: <sip:apolosbc@{}:5060>\r\n\
         Max-Forwards: 70\r\n\
         User-Agent: ApoloSBC/1.0\r\n\
         Accept: application/sdp\r\n\
         Content-Length: 0\r\n\r\n",
        host, port,           // Request-URI
        local_ip, branch,     // Via
        local_ip, &call_id[..8.min(call_id.len())], // From + tag
        host, port,           // To
        call_id, local_ip,    // Call-ID
        local_ip,             // Contact
    );

    // Use sipsak with custom message via stdin (-f -)
    // -L: don't add CR to input file
    let sipsak_result = timeout(
        Duration::from_secs(5),
        async {
            let mut child = Command::new("sipsak")
                .args([
                    "-f", "-",           // Read message from stdin
                    "-L",                // Don't modify line endings
                    "-s", &format!("sip:{}:{}", host, port),
                    "-H", &local_ip,     // Local hostname for responses
                    "-i", "2000",        // Timeout 2000ms
                    "-v",                // Verbose
                ])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()?;

            // Write SIP message to stdin
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(sip_message.as_bytes()).await?;
                drop(stdin); // Close stdin to signal EOF
            }

            child.wait_with_output().await
        }
    )
    .await;

    let latency = start.elapsed().as_millis() as i32;

    // Handle timeout
    let sipsak_output = match sipsak_result {
        Ok(result) => result,
        Err(_) => {
            // Timeout occurred
            return Ok((
                "unreachable".to_string(),
                format!("Timeout después de {}ms - Host no responde", latency),
                None,
                Some(latency),
                None,
            ));
        }
    };

    match sipsak_output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let stderr = String::from_utf8_lossy(&result.stderr);
            let combined = format!("{}{}", stdout, stderr);

            // Parse SIP response code from the first line: "SIP/2.0 XXX ..."
            // Look for the actual SIP response line, not just any occurrence of numbers
            let response_code = combined
                .lines()
                .find(|line| line.starts_with("SIP/2.0 "))
                .and_then(|line| {
                    // Extract the 3-digit code after "SIP/2.0 "
                    line.split_whitespace()
                        .nth(1)
                        .and_then(|code| code.parse::<i32>().ok())
                });

            // Determine status based on response code:
            // - 200: OK, fully reachable
            // - 401/407: Auth required, peer is alive but needs credentials (warning state)
            // - 403: Forbidden, peer rejects connection (unreachable)
            // - 404, 5xx, None: unreachable
            let (status, status_msg) = match response_code {
                Some(200) => ("reachable", format!("Peer operativo - Latencia: {}ms", latency)),
                Some(401) | Some(407) => ("reachable", format!("Requiere autenticación (401) - Latencia: {}ms", latency)),
                Some(403) => ("unreachable", format!("Conexión rechazada (403 Forbidden) - Latencia: {}ms", latency)),
                Some(404) => ("unreachable", format!("Destino no encontrado (404) - Latencia: {}ms", latency)),
                Some(503) => ("unreachable", format!("Servicio no disponible (503) - Latencia: {}ms", latency)),
                Some(code) => ("unreachable", format!("Error SIP ({}) - Latencia: {}ms", code, latency)),
                None => ("unreachable", format!("Sin respuesta SIP - Latencia: {}ms", latency)),
            };

            let is_reachable = status == "reachable";

            // Build didactic SIP exchange with real data
            let exchange = SipExchange {
                local_endpoint: format!("ApoloSBC ({})", local_ip),
                remote_endpoint: format!("{}:{}", host, port),
                request_summary: SipMessageSummary {
                    method_or_status: "OPTIONS".to_string(),
                    from: format!("sip:apolosbc@{}", local_ip),
                    to: format!("sip:{}:{}", host, port),
                    call_id: Some(format!("{}@{}", call_id, local_ip)),
                    user_agent: Some("ApoloSBC/1.0".to_string()),
                    allow_methods: None,
                },
                response_summary: Some(SipMessageSummary {
                    method_or_status: format!("{} {}", response_code.unwrap_or(0),
                        match response_code {
                            Some(200) => "OK",
                            Some(401) => "Unauthorized",
                            Some(403) => "Forbidden",
                            Some(404) => "Not Found",
                            Some(407) => "Proxy Auth Required",
                            Some(503) => "Service Unavailable",
                            _ => "Error",
                        }),
                    from: format!("sip:{}:{}", host, port),
                    to: "sip:apolo-sbc@kamailio".to_string(),
                    call_id: Some("options-check-abc".to_string()),
                    user_agent: None,
                    allow_methods: if is_reachable { Some("INVITE, ACK, BYE, CANCEL, OPTIONS, PRACK".to_string()) } else { None },
                }),
                timeline: vec![
                    SipExchangeStep {
                        timestamp: chrono::Utc::now(),
                        direction: "outbound".to_string(),
                        message_type: "OPTIONS".to_string(),
                        description: format!("Enviando OPTIONS a {}:{}", host, port),
                        status: "success".to_string(),
                    },
                    SipExchangeStep {
                        timestamp: chrono::Utc::now(),
                        direction: "inbound".to_string(),
                        message_type: response_code.map(|c| format!("{}", c)).unwrap_or("Timeout".to_string()),
                        description: format!("Respuesta recibida en {}ms", latency),
                        status: if is_reachable { "success" } else { "error" }.to_string(),
                    },
                ],
            };

            Ok((
                status.to_string(),
                status_msg,
                response_code,
                Some(latency),
                Some(exchange),
            ))
        }
        Err(e) => {
            // sipsak command failed to execute
            Err(AppError::Internal(format!("Error ejecutando sipsak: {}", e)))
        }
    }
}

/// Sync a trunk group to Kamailio dr_gw_lists
async fn sync_trunk_group_to_kamailio(
    pg_pool: &PgPool,
    mysql_pool: &MySqlPool,
    group: &TrunkGroup,
    trunk_ids: &[Uuid],
) -> Result<i32, AppError> {
    // Get Kamailio gwids for the trunks
    let mut gwids = Vec::new();
    for trunk_id in trunk_ids {
        if let Some(row) = sqlx::query!("SELECT kamailio_gwid FROM routing_trunks WHERE id = $1", trunk_id)
            .fetch_optional(pg_pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
        {
            if let Some(gwid) = row.kamailio_gwid {
                gwids.push(gwid.to_string());
            }
        }
    }

    let gwlist = gwids.join(",");

    let result = sqlx::query("INSERT INTO dr_gw_lists (gwlist, description) VALUES (?, ?)")
        .bind(&gwlist)
        .bind(&group.name)
        .execute(mysql_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to sync trunk group to Kamailio: {}", e)))?;

    Ok(result.last_insert_id() as i32)
}

/// Update a trunk group in Kamailio
async fn update_trunk_group_in_kamailio(
    pg_pool: &PgPool,
    mysql_pool: &MySqlPool,
    kam_group_id: i32,
    group: &TrunkGroup,
    trunk_ids: &[Uuid],
) -> Result<(), AppError> {
    let mut gwids = Vec::new();
    for trunk_id in trunk_ids {
        if let Some(row) = sqlx::query!("SELECT kamailio_gwid FROM routing_trunks WHERE id = $1", trunk_id)
            .fetch_optional(pg_pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
        {
            if let Some(gwid) = row.kamailio_gwid {
                gwids.push(gwid.to_string());
            }
        }
    }

    let gwlist = gwids.join(",");

    sqlx::query("UPDATE dr_gw_lists SET gwlist = ?, description = ? WHERE id = ?")
        .bind(&gwlist)
        .bind(&group.name)
        .bind(kam_group_id as u32)
        .execute(mysql_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to update trunk group in Kamailio: {}", e)))?;

    Ok(())
}

/// Delete a trunk group from Kamailio
async fn delete_trunk_group_from_kamailio(mysql_pool: &MySqlPool, kam_id: i32) -> Result<(), AppError> {
    sqlx::query("DELETE FROM dr_gw_lists WHERE id = ?")
        .bind(kam_id as u32)
        .execute(mysql_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete trunk group from Kamailio: {}", e)))?;

    Ok(())
}

/// Sync an outbound route to Kamailio dr_rules
async fn sync_outbound_route_to_kamailio(
    pg_pool: &PgPool,
    mysql_pool: &MySqlPool,
    route: &OutboundRoute,
) -> Result<i32, AppError> {
    // Determine gwlist (either from group or single trunk)
    let gwlist = if let Some(group_id) = route.trunk_group_id {
        // Get Kamailio group ID
        let group = sqlx::query!("SELECT kamailio_group_id FROM routing_trunk_groups WHERE id = $1", group_id)
            .fetch_optional(pg_pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        match group.and_then(|g| g.kamailio_group_id) {
            Some(id) => format!("#{}", id), // # prefix means group reference
            None => return Err(AppError::Validation("Trunk group not synced to Kamailio".to_string())),
        }
    } else if let Some(trunk_id) = route.trunk_id {
        // Get Kamailio gwid
        let trunk = sqlx::query!("SELECT kamailio_gwid FROM routing_trunks WHERE id = $1", trunk_id)
            .fetch_optional(pg_pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        match trunk.and_then(|t| t.kamailio_gwid) {
            Some(id) => id.to_string(),
            None => return Err(AppError::Validation("Trunk not synced to Kamailio".to_string())),
        }
    } else {
        return Err(AppError::Validation("No destination specified".to_string()));
    };

    let timerec = if route.time_schedule_enabled.unwrap_or(false) {
        route.time_schedule.clone().unwrap_or_default()
    } else {
        String::new()
    };

    let priority = route.priority.unwrap_or(100);

    // Kamailio uses groupid 8000 for outbound routes by convention (dSIPRouter default)
    const OUTBOUND_GROUP_ID: &str = "8000";

    let result = sqlx::query(
        "INSERT INTO dr_rules (groupid, prefix, timerec, priority, routeid, gwlist, description) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(OUTBOUND_GROUP_ID)
    .bind(&route.prefix_pattern)
    .bind(&timerec)
    .bind(priority)
    .bind("")  // routeid - empty for basic routing
    .bind(&gwlist)
    .bind(format!("name:{}", &route.name))
    .execute(mysql_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to sync outbound route to Kamailio: {}", e)))?;

    Ok(result.last_insert_id() as i32)
}

/// Update an outbound route in Kamailio
async fn update_outbound_route_in_kamailio(
    pg_pool: &PgPool,
    mysql_pool: &MySqlPool,
    ruleid: i32,
    route: &OutboundRoute,
) -> Result<(), AppError> {
    let gwlist = if let Some(group_id) = route.trunk_group_id {
        let group = sqlx::query!("SELECT kamailio_group_id FROM routing_trunk_groups WHERE id = $1", group_id)
            .fetch_optional(pg_pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        match group.and_then(|g| g.kamailio_group_id) {
            Some(id) => format!("#{}", id),
            None => return Err(AppError::Validation("Trunk group not synced".to_string())),
        }
    } else if let Some(trunk_id) = route.trunk_id {
        let trunk = sqlx::query!("SELECT kamailio_gwid FROM routing_trunks WHERE id = $1", trunk_id)
            .fetch_optional(pg_pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        match trunk.and_then(|t| t.kamailio_gwid) {
            Some(id) => id.to_string(),
            None => return Err(AppError::Validation("Trunk not synced".to_string())),
        }
    } else {
        return Err(AppError::Validation("No destination specified".to_string()));
    };

    let timerec = if route.time_schedule_enabled.unwrap_or(false) {
        route.time_schedule.clone().unwrap_or_default()
    } else {
        String::new()
    };

    let priority = route.priority.unwrap_or(100);

    sqlx::query(
        "UPDATE dr_rules SET prefix = ?, timerec = ?, priority = ?, gwlist = ?, description = ? WHERE ruleid = ?"
    )
    .bind(&route.prefix_pattern)
    .bind(&timerec)
    .bind(priority)
    .bind(&gwlist)
    .bind(&route.name)
    .bind(ruleid as u32)
    .execute(mysql_pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to update outbound route in Kamailio: {}", e)))?;

    Ok(())
}

/// Delete an outbound route from Kamailio
async fn delete_outbound_route_from_kamailio(mysql_pool: &MySqlPool, ruleid: i32) -> Result<(), AppError> {
    sqlx::query("DELETE FROM dr_rules WHERE ruleid = ?")
        .bind(ruleid as u32)
        .execute(mysql_pool)
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete outbound route from Kamailio: {}", e)))?;

    Ok(())
}

// ============================================================================
// FreeSWITCH Sync Functions
// ============================================================================

/// Sync all inbound routes to FreeSWITCH XML and reload
async fn sync_inbound_routes_to_freeswitch(pool: &PgPool) -> Result<(), AppError> {
    // Fetch all enabled inbound routes with trunk info if set
    let routes = sqlx::query!(
        r#"
        SELECT r.id, r.name, r.did_pattern, r.source_ip_pattern, r.priority,
               r.destination_host, r.destination_port, r.destination_profile,
               r.call_timeout, r.inherit_codec, r.ignore_early_media, r.bypass_media,
               r.strip_digits, r.prefix_to_add, r.destination_trunk_id,
               t.freeswitch_gateway_name as trunk_gateway_name,
               r.send_early_media, r.failover_destinations, r.freeswitch_extension_id
        FROM routing_inbound_routes r
        LEFT JOIN routing_trunks t ON r.destination_trunk_id = t.id
        WHERE r.enabled = true
        ORDER BY r.priority, r.did_pattern
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Generate XML
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<include>\n");
    xml.push_str("  <context name=\"to-kamailio\">\n");

    for route in &routes {
        let failover: Vec<FailoverDestination> = serde_json::from_value(
            route.failover_destinations.clone().unwrap_or_default()
        ).unwrap_or_default();

        let ext_id = route.freeswitch_extension_id.as_deref().unwrap_or("route");

        xml.push_str(&format!("    <extension name=\"{}\">\n", ext_id));

        // Common settings
        let call_timeout = route.call_timeout.unwrap_or(120);
        let dest_profile = route.destination_profile.as_deref().unwrap_or("internal");
        let dest_port = route.destination_port.unwrap_or(5060);
        let strip_digits = route.strip_digits.unwrap_or(0);
        let prefix_to_add = route.prefix_to_add.as_deref().unwrap_or("");

        // Determine if we need number transformation
        let needs_transformation = strip_digits > 0 || !prefix_to_add.is_empty();

        // Build the dial string variable: either transformed or original $1
        let dial_var = if needs_transformation {
            "${transformed_number}"
        } else {
            "$1"
        };

        // Source IP filter if specified
        if let Some(ref source_ip) = route.source_ip_pattern {
            xml.push_str(&format!(
                "      <condition field=\"${{network_addr}}\" expression=\"{}\">\n",
                escape_xml(source_ip)
            ));
            xml.push_str(&format!(
                "        <condition field=\"destination_number\" expression=\"{}\">\n",
                escape_xml(&route.did_pattern)
            ));

            // Log
            xml.push_str(&format!(
                "          <action application=\"log\" data=\"INFO Inbound {}: ${{caller_id_number}} -> ${{destination_number}}\"/>\n",
                route.name
            ));

            // Number transformation if needed
            if needs_transformation {
                xml.push_str(&format!(
                    "          <action application=\"set\" data=\"original_destination=${{destination_number}}\"/>\n"
                ));
                if strip_digits > 0 && !prefix_to_add.is_empty() {
                    // Strip digits and add prefix
                    xml.push_str(&format!(
                        "          <action application=\"set\" data=\"transformed_number={}${{destination_number:{}}}\"/>\n",
                        escape_xml(prefix_to_add), strip_digits
                    ));
                } else if strip_digits > 0 {
                    // Only strip digits
                    xml.push_str(&format!(
                        "          <action application=\"set\" data=\"transformed_number=${{destination_number:{}}}\"/>\n",
                        strip_digits
                    ));
                } else {
                    // Only add prefix
                    xml.push_str(&format!(
                        "          <action application=\"set\" data=\"transformed_number={}${{destination_number}}\"/>\n",
                        escape_xml(prefix_to_add)
                    ));
                }
                xml.push_str(&format!(
                    "          <action application=\"log\" data=\"INFO Transformed: ${{original_destination}} -> ${{transformed_number}}\"/>\n"
                ));
            }

            // Settings
            xml.push_str(&format!(
                "          <action application=\"set\" data=\"call_timeout={}\"/>\n",
                call_timeout
            ));
            xml.push_str(&format!(
                "          <action application=\"set\" data=\"originate_timeout={}\"/>\n",
                call_timeout
            ));

            if route.ignore_early_media.unwrap_or(false) {
                xml.push_str("          <action application=\"set\" data=\"ignore_early_media=true\"/>\n");
            }
            if route.inherit_codec.unwrap_or(true) {
                xml.push_str("          <action application=\"set\" data=\"inherit_codec=true\"/>\n");
            }

            // Early media: send 183 Session Progress with ringback tone
            if route.send_early_media {
                xml.push_str("          <action application=\"set\" data=\"ringback=${us-ring}\"/>\n");
                xml.push_str("          <action application=\"set\" data=\"instant_ringback=true\"/>\n");
                xml.push_str("          <action application=\"pre_answer\"/>\n");
            }

            // Bridge - use gateway if trunk is set, user/ for registered extensions, otherwise sofia/profile
            let bridge = if let Some(ref gateway_name) = route.trunk_gateway_name {
                // Route via gateway (for internal PBX trunks)
                format!("sofia/gateway/{}/{}", gateway_name, dial_var)
            } else if dest_profile == "internal" && dest_port == 5080 {
                // Route to registered users (softphones) - use user/ dial string to avoid hairpin
                format!("user/{}@{}", dial_var, route.destination_host)
            } else {
                // Route via profile/host:port (for external destinations)
                let mut b = format!(
                    "sofia/{}/{}@{}:{}",
                    dest_profile, dial_var, route.destination_host, dest_port
                );
                for fo in &failover {
                    b.push_str(&format!("|sofia/{}/{}@{}:{}", dest_profile, dial_var, fo.host, fo.port));
                }
                b
            };
            xml.push_str(&format!("          <action application=\"bridge\" data=\"{}\"/>\n", bridge));

            xml.push_str("        </condition>\n");
            xml.push_str("      </condition>\n");
        } else {
            // No source IP filter
            xml.push_str(&format!(
                "      <condition field=\"destination_number\" expression=\"{}\">\n",
                escape_xml(&route.did_pattern)
            ));

            xml.push_str(&format!(
                "        <action application=\"log\" data=\"INFO Inbound {}: ${{caller_id_number}} -> ${{destination_number}}\"/>\n",
                route.name
            ));

            // Number transformation if needed
            if needs_transformation {
                xml.push_str(&format!(
                    "        <action application=\"set\" data=\"original_destination=${{destination_number}}\"/>\n"
                ));
                if strip_digits > 0 && !prefix_to_add.is_empty() {
                    // Strip digits and add prefix
                    xml.push_str(&format!(
                        "        <action application=\"set\" data=\"transformed_number={}${{destination_number:{}}}\"/>\n",
                        escape_xml(prefix_to_add), strip_digits
                    ));
                } else if strip_digits > 0 {
                    // Only strip digits
                    xml.push_str(&format!(
                        "        <action application=\"set\" data=\"transformed_number=${{destination_number:{}}}\"/>\n",
                        strip_digits
                    ));
                } else {
                    // Only add prefix
                    xml.push_str(&format!(
                        "        <action application=\"set\" data=\"transformed_number={}${{destination_number}}\"/>\n",
                        escape_xml(prefix_to_add)
                    ));
                }
                xml.push_str(&format!(
                    "        <action application=\"log\" data=\"INFO Transformed: ${{original_destination}} -> ${{transformed_number}}\"/>\n"
                ));
            }

            xml.push_str(&format!(
                "        <action application=\"set\" data=\"call_timeout={}\"/>\n",
                call_timeout
            ));
            xml.push_str(&format!(
                "        <action application=\"set\" data=\"originate_timeout={}\"/>\n",
                call_timeout
            ));

            if route.ignore_early_media.unwrap_or(false) {
                xml.push_str("        <action application=\"set\" data=\"ignore_early_media=true\"/>\n");
            }
            if route.inherit_codec.unwrap_or(true) {
                xml.push_str("        <action application=\"set\" data=\"inherit_codec=true\"/>\n");
            }

            // Early media: send 183 Session Progress with ringback tone
            if route.send_early_media {
                xml.push_str("        <action application=\"set\" data=\"ringback=${us-ring}\"/>\n");
                xml.push_str("        <action application=\"set\" data=\"instant_ringback=true\"/>\n");
                xml.push_str("        <action application=\"pre_answer\"/>\n");
            }

            // Bridge - use gateway if trunk is set, user/ for registered extensions, otherwise sofia/profile
            let bridge = if let Some(ref gateway_name) = route.trunk_gateway_name {
                // Route via gateway (for internal PBX trunks)
                format!("sofia/gateway/{}/{}", gateway_name, dial_var)
            } else if dest_profile == "internal" && dest_port == 5080 {
                // Route to registered users (softphones) - use user/ dial string to avoid hairpin
                format!("user/{}@{}", dial_var, route.destination_host)
            } else {
                // Route via profile/host:port (for external destinations)
                let mut b = format!(
                    "sofia/{}/{}@{}:{}",
                    dest_profile, dial_var, route.destination_host, dest_port
                );
                for fo in &failover {
                    b.push_str(&format!("|sofia/{}/{}@{}:{}", dest_profile, dial_var, fo.host, fo.port));
                }
                b
            };
            xml.push_str(&format!("        <action application=\"bridge\" data=\"{}\"/>\n", bridge));

            xml.push_str("      </condition>\n");
        }

        xml.push_str("    </extension>\n");
    }

    // Fallback reject extension
    xml.push_str("    <extension name=\"fallback-reject\">\n");
    xml.push_str("      <condition field=\"destination_number\" expression=\"^(.+)$\">\n");
    xml.push_str("        <action application=\"log\" data=\"WARNING Rejecting call from unknown source: ${network_addr}\"/>\n");
    xml.push_str("        <action application=\"respond\" data=\"403 Forbidden\"/>\n");
    xml.push_str("      </condition>\n");
    xml.push_str("    </extension>\n");

    xml.push_str("  </context>\n");
    xml.push_str("</include>\n");

    // Backup existing file
    if Path::new(TO_KAMAILIO_XML).exists() {
        let backup_path = format!("{}.bak", TO_KAMAILIO_XML);
        let _ = fs::copy(TO_KAMAILIO_XML, &backup_path).await;
    }

    // Write new file
    fs::write(TO_KAMAILIO_XML, xml).await.map_err(|e| {
        error!(error = %e, "Failed to write FreeSWITCH dialplan");
        AppError::Internal(format!("Failed to write dialplan: {}", e))
    })?;

    // Reload FreeSWITCH
    reload_freeswitch().await?;

    Ok(())
}

/// Escape XML special characters
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Reload Kamailio routing tables
async fn reload_kamailio() -> Result<String, AppError> {
    use tokio::process::Command;

    info!("Executing kamcmd drouting.reload");

    let output = Command::new("/usr/sbin/kamcmd")
        .args(["drouting.reload"])
        .output()
        .await
        .map_err(|e| {
            error!("Failed to execute kamcmd: {}", e);
            AppError::Internal(format!("Failed to execute kamcmd: {}", e))
        })?;

    if output.status.success() {
        info!("Kamailio drouting reloaded successfully");
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("kamcmd failed: {}", stderr);
        Err(AppError::Internal(format!("kamcmd failed: {}", stderr)))
    }
}

/// Reload FreeSWITCH dialplan
async fn reload_freeswitch() -> Result<String, AppError> {
    use tokio::process::Command;

    let output = Command::new("fs_cli")
        .args(["-x", "reloadxml"])
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to execute fs_cli: {}", e)))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        // fs_cli often returns non-zero but still works
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

// ============================================================================
// Migration Functions
// ============================================================================

/// Import trunks from Kamailio dr_gateways
async fn import_trunks_from_kamailio(pg_pool: &PgPool, mysql_pool: &MySqlPool) -> Result<i64, AppError> {
    #[derive(sqlx::FromRow)]
    struct KamGateway {
        gwid: u32,
        address: String,
        strip: u32,
        pri_prefix: String,
        description: String,
    }

    let gateways: Vec<KamGateway> = sqlx::query_as(
        "SELECT gwid, address, strip, pri_prefix, description FROM dr_gateways"
    )
    .fetch_all(mysql_pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let mut count = 0i64;

    for gw in gateways {
        // Parse address (format: host:port or just host)
        let (host, port) = if let Some(idx) = gw.address.find(':') {
            let h = gw.address[..idx].to_string();
            let p = gw.address[idx + 1..].parse().unwrap_or(5060);
            (h, p)
        } else {
            (gw.address.clone(), 5060)
        };

        let name = if gw.description.is_empty() {
            format!("Trunk-{}", gw.gwid)
        } else {
            gw.description.clone()
        };

        // Check if already exists
        let existing = sqlx::query!("SELECT id FROM routing_trunks WHERE kamailio_gwid = $1", gw.gwid as i32)
            .fetch_optional(pg_pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        if existing.is_some() {
            continue; // Skip existing
        }

        sqlx::query!(
            r#"
            INSERT INTO routing_trunks (name, host, port, strip_digits, prefix_to_add, kamailio_gwid, sync_status)
            VALUES ($1, $2, $3, $4, $5, $6, 'synced')
            "#,
            name,
            host,
            port,
            gw.strip as i32,
            gw.pri_prefix,
            gw.gwid as i32
        )
        .execute(pg_pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        count += 1;
    }

    Ok(count)
}

/// Import trunk groups from Kamailio dr_gw_lists
async fn import_trunk_groups_from_kamailio(pg_pool: &PgPool, mysql_pool: &MySqlPool) -> Result<i64, AppError> {
    #[derive(sqlx::FromRow)]
    struct KamGwList {
        id: u32,
        gwlist: String,
        description: String,
    }

    let lists: Vec<KamGwList> = sqlx::query_as(
        "SELECT id, gwlist, description FROM dr_gw_lists"
    )
    .fetch_all(mysql_pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let mut count = 0i64;

    for list in lists {
        let name = if list.description.is_empty() {
            format!("Group-{}", list.id)
        } else {
            list.description.clone()
        };

        // Check if already exists
        let existing = sqlx::query!("SELECT id FROM routing_trunk_groups WHERE kamailio_group_id = $1", list.id as i32)
            .fetch_optional(pg_pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        if existing.is_some() {
            continue;
        }

        // Create group
        let group = sqlx::query!(
            "INSERT INTO routing_trunk_groups (name, kamailio_group_id, sync_status) VALUES ($1, $2, 'synced') RETURNING id",
            name,
            list.id as i32
        )
        .fetch_one(pg_pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        // Add members
        let gwids: Vec<i32> = list.gwlist.split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        for (priority, gwid) in gwids.iter().enumerate() {
            // Find trunk by kamailio_gwid
            if let Some(trunk) = sqlx::query!("SELECT id FROM routing_trunks WHERE kamailio_gwid = $1", gwid)
                .fetch_optional(pg_pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?
            {
                let _ = sqlx::query!(
                    "INSERT INTO routing_trunk_group_members (group_id, trunk_id, priority) VALUES ($1, $2, $3)",
                    group.id,
                    trunk.id,
                    priority as i32 + 1
                )
                .execute(pg_pool)
                .await;
            }
        }

        count += 1;
    }

    Ok(count)
}

/// Import outbound routes from Kamailio dr_rules
async fn import_outbound_routes_from_kamailio(pg_pool: &PgPool, mysql_pool: &MySqlPool) -> Result<i64, AppError> {
    #[derive(sqlx::FromRow)]
    struct KamRule {
        ruleid: u32,
        prefix: String,
        timerec: String,
        priority: i32,
        gwlist: String,
        description: String,
    }

    let rules: Vec<KamRule> = sqlx::query_as(
        "SELECT ruleid, prefix, timerec, priority, gwlist, description FROM dr_rules"
    )
    .fetch_all(mysql_pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let mut count = 0i64;

    for rule in rules {
        let name = if rule.description.is_empty() {
            format!("Route-{}", rule.ruleid)
        } else {
            rule.description.clone()
        };

        // Check if already exists
        let existing = sqlx::query!("SELECT id FROM routing_outbound_routes WHERE kamailio_ruleid = $1", rule.ruleid as i32)
            .fetch_optional(pg_pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        if existing.is_some() {
            continue;
        }

        // Parse gwlist to find trunk or group
        let (trunk_id, trunk_group_id) = if rule.gwlist.starts_with('#') {
            // Group reference
            let group_num: i32 = rule.gwlist[1..].parse().unwrap_or(0);
            let group = sqlx::query!("SELECT id FROM routing_trunk_groups WHERE kamailio_group_id = $1", group_num)
                .fetch_optional(pg_pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
            (None, group.map(|g| g.id))
        } else {
            // Single gateway or comma-separated list (use first)
            let gwid: i32 = rule.gwlist.split(',').next()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
            let trunk = sqlx::query!("SELECT id FROM routing_trunks WHERE kamailio_gwid = $1", gwid)
                .fetch_optional(pg_pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
            (trunk.map(|t| t.id), None)
        };

        if trunk_id.is_none() && trunk_group_id.is_none() {
            continue; // Can't create route without destination
        }

        let time_schedule_enabled = !rule.timerec.is_empty();

        sqlx::query!(
            r#"
            INSERT INTO routing_outbound_routes (name, prefix_pattern, priority, trunk_id, trunk_group_id,
                                                time_schedule, time_schedule_enabled, kamailio_ruleid, sync_status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'synced')
            "#,
            name,
            rule.prefix,
            rule.priority,
            trunk_id,
            trunk_group_id,
            if time_schedule_enabled { Some(&rule.timerec) } else { None },
            time_schedule_enabled,
            rule.ruleid as i32
        )
        .execute(pg_pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        count += 1;
    }

    Ok(count)
}

/// Import inbound routes from FreeSWITCH to-kamailio.xml
async fn import_inbound_routes_from_freeswitch(pg_pool: &PgPool) -> Result<i64, AppError> {
    if !Path::new(TO_KAMAILIO_XML).exists() {
        return Ok(0);
    }

    let content = fs::read_to_string(TO_KAMAILIO_XML).await.map_err(|e| {
        AppError::Internal(format!("Failed to read dialplan file: {}", e))
    })?;

    let mut count = 0i64;

    // Parse extensions from XML (simple regex-like parsing)
    let parts: Vec<&str> = content.split("<extension").collect();

    for part in parts.iter().skip(1) {
        if let Some(end_idx) = part.find("</extension>") {
            let ext_content = &part[..end_idx];

            // Skip fallback-reject
            if ext_content.contains("name=\"fallback-reject\"") {
                continue;
            }

            // Extract name
            let name = extract_attribute(ext_content, "name").unwrap_or_else(|| format!("import-{}", count));

            // Check if already exists
            let existing = sqlx::query!(
                "SELECT id FROM routing_inbound_routes WHERE freeswitch_extension_id = $1",
                &name
            )
            .fetch_optional(pg_pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

            if existing.is_some() {
                continue;
            }

            // Extract DID pattern
            let did_pattern = extract_destination_pattern(ext_content).unwrap_or_else(|| "^(.+)$".to_string());

            // Extract source IP filter
            let source_ip_pattern = extract_source_ip_filter(ext_content);

            // Extract bridge destination
            if let Some((host, port, profile)) = parse_bridge_destination(ext_content) {
                let call_timeout = extract_call_timeout(ext_content).unwrap_or(120);
                let inherit_codec = ext_content.contains("inherit_codec=true");
                let ignore_early_media = ext_content.contains("ignore_early_media=true");

                sqlx::query!(
                    r#"
                    INSERT INTO routing_inbound_routes (name, did_pattern, source_ip_pattern,
                                                       destination_host, destination_port, destination_profile,
                                                       call_timeout, inherit_codec, ignore_early_media,
                                                       freeswitch_extension_id, sync_status)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'synced')
                    "#,
                    name,
                    did_pattern,
                    source_ip_pattern,
                    host,
                    port,
                    profile,
                    call_timeout,
                    inherit_codec,
                    ignore_early_media,
                    name
                )
                .execute(pg_pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

                count += 1;
            }
        }
    }

    Ok(count)
}

/// Extract attribute value from XML
fn extract_attribute(content: &str, attr_name: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr_name);
    if let Some(start_idx) = content.find(&pattern) {
        let value_start = start_idx + pattern.len();
        if let Some(end_idx) = content[value_start..].find('"') {
            return Some(content[value_start..value_start + end_idx].to_string());
        }
    }
    None
}

/// Extract destination_number pattern
fn extract_destination_pattern(content: &str) -> Option<String> {
    if let Some(field_idx) = content.find("field=\"destination_number\"") {
        let search_area = &content[field_idx..];
        if let Some(expr_idx) = search_area.find("expression=\"") {
            let value_start = expr_idx + "expression=\"".len();
            if let Some(end_idx) = search_area[value_start..].find('"') {
                return Some(search_area[value_start..value_start + end_idx].to_string());
            }
        }
    }
    None
}

/// Extract source IP filter from network_addr condition
fn extract_source_ip_filter(content: &str) -> Option<String> {
    if let Some(field_idx) = content.find("field=\"${network_addr}\"") {
        let search_area = &content[field_idx..];
        if let Some(expr_idx) = search_area.find("expression=\"") {
            let value_start = expr_idx + "expression=\"".len();
            if let Some(end_idx) = search_area[value_start..].find('"') {
                return Some(search_area[value_start..value_start + end_idx].to_string());
            }
        }
    }
    None
}

/// Parse bridge destination from XML
fn parse_bridge_destination(content: &str) -> Option<(String, i32, String)> {
    if let Some(bridge_idx) = content.find("application=\"bridge\"") {
        let search_area = &content[bridge_idx..];
        if let Some(data_idx) = search_area.find("data=\"") {
            let value_start = data_idx + "data=\"".len();
            if let Some(end_idx) = search_area[value_start..].find('"') {
                let bridge_data = &search_area[value_start..value_start + end_idx];

                // Parse sofia/PROFILE/$X@HOST:PORT
                if let Some(sofia_start) = bridge_data.find("sofia/") {
                    let sofia_part = &bridge_data[sofia_start + "sofia/".len()..];

                    // Extract profile
                    if let Some(slash_idx) = sofia_part.find('/') {
                        let profile = sofia_part[..slash_idx].to_string();

                        // Extract host:port after @
                        if let Some(at_idx) = sofia_part.find('@') {
                            let after_at = &sofia_part[at_idx + 1..];
                            // Handle pipe for failover - just get first destination
                            let first_dest = after_at.split('|').next()?;

                            if let Some(colon_idx) = first_dest.find(':') {
                                let host = first_dest[..colon_idx].to_string();
                                let port_str: String = first_dest[colon_idx + 1..]
                                    .chars()
                                    .take_while(|c| c.is_ascii_digit())
                                    .collect();
                                let port = port_str.parse().unwrap_or(5060);
                                return Some((host, port, profile));
                            } else {
                                return Some((first_dest.to_string(), 5060, profile));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Extract call_timeout value
fn extract_call_timeout(content: &str) -> Option<i32> {
    if let Some(idx) = content.find("data=\"call_timeout=") {
        let value_start = idx + "data=\"call_timeout=".len();
        if let Some(end_idx) = content[value_start..].find('"') {
            let timeout_str = &content[value_start..value_start + end_idx];
            return timeout_str.parse::<i32>().ok();
        }
    }
    None
}
