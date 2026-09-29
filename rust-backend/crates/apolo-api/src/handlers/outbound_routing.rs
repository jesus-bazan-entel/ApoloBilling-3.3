//! Outbound Routing handlers
//!
//! HTTP handlers for carrier groups and outbound route management.
//! Routes are matched by prefix with auto-priority based on prefix length.

use crate::dto::carriers::SyncStatus;
use crate::dto::outbound::{
    AddCarrierGroupMemberRequest, CarrierGroup, CarrierGroupListResponse, CarrierGroupMember,
    CarrierGroupSyncResult, CarrierGroupWithMembers, CreateCarrierGroupRequest,
    CreateOutboundRouteRequest, FailoverStrategy, OutboundRoute, OutboundRouteListResponse,
    OutboundRouteSyncResult, OutboundSyncAllResult, UpdateCarrierGroupMemberRequest,
    UpdateCarrierGroupRequest, UpdateOutboundRouteRequest,
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
// Carrier Group Handlers
// ============================================================================

/// List all carrier groups with members
///
/// GET /api/v1/carrier-groups
#[instrument(skip(pool, user))]
pub async fn list_carrier_groups(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("carrier_groups") {
        return Err(AppError::Forbidden);
    }

    debug!("Listing all carrier groups");

    let groups = sqlx::query!(
        r#"
        SELECT id, name, description, strategy,
               kamailio_group_id, sync_status, sync_error,
               created_at, updated_at
        FROM carrier_groups
        ORDER BY name
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let mut response = Vec::new();

    for g in groups {
        let members = sqlx::query!(
            r#"
            SELECT m.id, m.group_id, m.carrier_id, m.priority, m.weight,
                   c.name as carrier_name, c.host as carrier_host, c.enabled as carrier_enabled
            FROM carrier_group_members m
            JOIN carriers c ON m.carrier_id = c.id
            WHERE m.group_id = $1
            ORDER BY m.priority, m.weight DESC
            "#,
            g.id
        )
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        let group = CarrierGroup {
            id: g.id,
            name: g.name,
            description: g.description,
            strategy: g
                .strategy
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(FailoverStrategy::Sequential),
            kamailio_group_id: g.kamailio_group_id,
            sync_status: g
                .sync_status
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(SyncStatus::Pending),
            sync_error: g.sync_error,
            created_at: g.created_at.unwrap_or_else(chrono::Utc::now),
            updated_at: g.updated_at.unwrap_or_else(chrono::Utc::now),
        };

        let members: Vec<CarrierGroupMember> = members
            .into_iter()
            .map(|m| CarrierGroupMember {
                id: m.id,
                group_id: m.group_id,
                carrier_id: m.carrier_id,
                carrier_name: m.carrier_name,
                carrier_host: m.carrier_host,
                carrier_enabled: m.carrier_enabled.unwrap_or(true),
                priority: m.priority.unwrap_or(1),
                weight: m.weight.unwrap_or(100),
            })
            .collect();

        response.push(CarrierGroupWithMembers { group, members });
    }

    let total = response.len() as i64;

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(CarrierGroupListResponse {
            groups: response,
            total,
        })),
    )
}

/// Get a single carrier group by ID
///
/// GET /api/v1/carrier-groups/{id}
#[instrument(skip(pool, user))]
pub async fn get_carrier_group(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("carrier_groups") {
        return Err(AppError::Forbidden);
    }

    let group_id = path.into_inner();
    debug!(id = %group_id, "Getting carrier group");

    let g = sqlx::query!(
        r#"
        SELECT id, name, description, strategy,
               kamailio_group_id, sync_status, sync_error,
               created_at, updated_at
        FROM carrier_groups
        WHERE id = $1
        "#,
        group_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Carrier group {} not found", group_id)))?;

    let members = sqlx::query!(
        r#"
        SELECT m.id, m.group_id, m.carrier_id, m.priority, m.weight,
               c.name as carrier_name, c.host as carrier_host, c.enabled as carrier_enabled
        FROM carrier_group_members m
        JOIN carriers c ON m.carrier_id = c.id
        WHERE m.group_id = $1
        ORDER BY m.priority, m.weight DESC
        "#,
        group_id
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let group = CarrierGroup {
        id: g.id,
        name: g.name,
        description: g.description,
        strategy: g
            .strategy
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(FailoverStrategy::Sequential),
        kamailio_group_id: g.kamailio_group_id,
        sync_status: g
            .sync_status
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(SyncStatus::Pending),
        sync_error: g.sync_error,
        created_at: g.created_at.unwrap_or_else(chrono::Utc::now),
        updated_at: g.updated_at.unwrap_or_else(chrono::Utc::now),
    };

    let members: Vec<CarrierGroupMember> = members
        .into_iter()
        .map(|m| CarrierGroupMember {
            id: m.id,
            group_id: m.group_id,
            carrier_id: m.carrier_id,
            carrier_name: m.carrier_name,
            carrier_host: m.carrier_host,
            carrier_enabled: m.carrier_enabled.unwrap_or(true),
            priority: m.priority.unwrap_or(1),
            weight: m.weight.unwrap_or(100),
        })
        .collect();

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(CarrierGroupWithMembers {
            group,
            members,
        })),
    )
}

/// Create a new carrier group
///
/// POST /api/v1/carrier-groups
#[instrument(skip(pool, admin, req))]
pub async fn create_carrier_group(
    pool: web::Data<PgPool>,
    admin: AdminUser,
    req: web::Json<CreateCarrierGroupRequest>,
) -> Result<HttpResponse, AppError> {
    info!(name = %req.name, "Creating carrier group");

    if req.name.trim().is_empty() {
        return Err(AppError::Validation(
            "Group name cannot be empty".to_string(),
        ));
    }

    // Check for duplicate name
    let existing = sqlx::query!("SELECT id FROM carrier_groups WHERE name = $1", req.name)
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    if existing.is_some() {
        return Err(AppError::AlreadyExists(format!(
            "Carrier group with name '{}' already exists",
            req.name
        )));
    }

    let group_id = Uuid::new_v4();
    let strategy = req.strategy.clone().unwrap_or(FailoverStrategy::Sequential);

    sqlx::query!(
        r#"
        INSERT INTO carrier_groups (id, name, description, strategy, sync_status)
        VALUES ($1, $2, $3, $4, 'pending')
        "#,
        group_id,
        req.name,
        req.description,
        strategy.to_string(),
    )
    .execute(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("create_carrier_group")
        .entity_type("carrier_group")
        .entity_id(group_id.to_string())
        .details(serde_json::json!({"name": req.name}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    get_carrier_group(pool, web::Path::from(group_id), admin.0).await
}

/// Update a carrier group
///
/// PUT /api/v1/carrier-groups/{id}
#[instrument(skip(pool, admin, req))]
pub async fn update_carrier_group(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    admin: AdminUser,
    req: web::Json<UpdateCarrierGroupRequest>,
) -> Result<HttpResponse, AppError> {
    let group_id = path.into_inner();
    info!(id = %group_id, "Updating carrier group");

    // Check exists
    let _ = sqlx::query!("SELECT id FROM carrier_groups WHERE id = $1", group_id)
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Carrier group {} not found", group_id)))?;

    // Check for duplicate name if changing
    if let Some(ref name) = req.name {
        let dup = sqlx::query!(
            "SELECT id FROM carrier_groups WHERE name = $1 AND id != $2",
            name,
            group_id
        )
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        if dup.is_some() {
            return Err(AppError::AlreadyExists(format!(
                "Carrier group with name '{}' already exists",
                name
            )));
        }
    }

    let strategy_str = req.strategy.as_ref().map(|s| s.to_string());

    sqlx::query!(
        r#"
        UPDATE carrier_groups SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            strategy = COALESCE($4, strategy),
            sync_status = 'pending',
            updated_at = NOW()
        WHERE id = $1
        "#,
        group_id,
        req.name,
        req.description,
        strategy_str,
    )
    .execute(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("update_carrier_group")
        .entity_type("carrier_group")
        .entity_id(group_id.to_string())
        .details(serde_json::json!({"updated": true}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    get_carrier_group(pool, web::Path::from(group_id), admin.0).await
}

/// Delete a carrier group
///
/// DELETE /api/v1/carrier-groups/{id}
#[instrument(skip(pool, admin))]
pub async fn delete_carrier_group(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let group_id = path.into_inner();
    info!(id = %group_id, "Deleting carrier group");

    // Check if used by any outbound routes
    let in_use = sqlx::query!(
        "SELECT id FROM outbound_routes WHERE carrier_group_id = $1 LIMIT 1",
        group_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    if in_use.is_some() {
        return Err(AppError::Conflict(
            "Cannot delete carrier group that is used by outbound routes".to_string(),
        ));
    }

    // Get kamailio_group_id before deletion
    let group = sqlx::query!(
        "SELECT kamailio_group_id FROM carrier_groups WHERE id = $1",
        group_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Carrier group {} not found", group_id)))?;

    // Delete from PostgreSQL (cascade deletes members)
    let result = sqlx::query!("DELETE FROM carrier_groups WHERE id = $1", group_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Carrier group {} not found",
            group_id
        )));
    }

    // Delete from Kamailio if synced
    if let Some(kam_id) = group.kamailio_group_id {
        if let Err(e) = delete_kamailio_group(kam_id).await {
            warn!(kam_id = kam_id, error = %e, "Failed to delete group from Kamailio");
        }
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("delete_carrier_group")
        .entity_type("carrier_group")
        .entity_id(group_id.to_string())
        .details(serde_json::json!({"deleted": true}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        (),
        "Carrier group deleted successfully",
    )))
}

/// Add a member to a carrier group
///
/// POST /api/v1/carrier-groups/{id}/members
#[instrument(skip(pool, admin, req))]
pub async fn add_carrier_group_member(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    admin: AdminUser,
    req: web::Json<AddCarrierGroupMemberRequest>,
) -> Result<HttpResponse, AppError> {
    let group_id = path.into_inner();
    info!(group_id = %group_id, carrier_id = %req.carrier_id, "Adding member to carrier group");

    // Check group exists
    let _ = sqlx::query!("SELECT id FROM carrier_groups WHERE id = $1", group_id)
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Carrier group {} not found", group_id)))?;

    // Check carrier exists
    let _ = sqlx::query!("SELECT id FROM carriers WHERE id = $1", req.carrier_id)
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Carrier {} not found", req.carrier_id)))?;

    // Check not already a member
    let existing = sqlx::query!(
        "SELECT id FROM carrier_group_members WHERE group_id = $1 AND carrier_id = $2",
        group_id,
        req.carrier_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    if existing.is_some() {
        return Err(AppError::AlreadyExists(
            "Carrier is already a member of this group".to_string(),
        ));
    }

    let member_id = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO carrier_group_members (id, group_id, carrier_id, priority, weight)
        VALUES ($1, $2, $3, $4, $5)
        "#,
        member_id,
        group_id,
        req.carrier_id,
        req.priority.unwrap_or(1),
        req.weight.unwrap_or(100),
    )
    .execute(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Mark group as needing sync
    sqlx::query!(
        "UPDATE carrier_groups SET sync_status = 'pending' WHERE id = $1",
        group_id
    )
    .execute(pool.get_ref())
    .await
    .ok();

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("add_carrier_group_member")
        .entity_type("carrier_group_member")
        .entity_id(member_id.to_string())
        .details(serde_json::json!({
            "group_id": group_id,
            "carrier_id": req.carrier_id,
        }))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    get_carrier_group(pool, web::Path::from(group_id), admin.0).await
}

/// Update a carrier group member
///
/// PUT /api/v1/carrier-groups/{group_id}/members/{member_id}
#[instrument(skip(pool, admin, req))]
pub async fn update_carrier_group_member(
    pool: web::Data<PgPool>,
    path: web::Path<(Uuid, Uuid)>,
    admin: AdminUser,
    req: web::Json<UpdateCarrierGroupMemberRequest>,
) -> Result<HttpResponse, AppError> {
    let (group_id, member_id) = path.into_inner();
    info!(group_id = %group_id, member_id = %member_id, "Updating carrier group member");

    sqlx::query!(
        r#"
        UPDATE carrier_group_members SET
            priority = COALESCE($2, priority),
            weight = COALESCE($3, weight)
        WHERE id = $1 AND group_id = $4
        "#,
        member_id,
        req.priority,
        req.weight,
        group_id,
    )
    .execute(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Mark group as needing sync
    sqlx::query!(
        "UPDATE carrier_groups SET sync_status = 'pending' WHERE id = $1",
        group_id
    )
    .execute(pool.get_ref())
    .await
    .ok();

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("update_carrier_group_member")
        .entity_type("carrier_group_member")
        .entity_id(member_id.to_string())
        .details(serde_json::json!({"updated": true}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    get_carrier_group(pool, web::Path::from(group_id), admin.0).await
}

/// Remove a member from a carrier group
///
/// DELETE /api/v1/carrier-groups/{group_id}/members/{member_id}
#[instrument(skip(pool, admin))]
pub async fn remove_carrier_group_member(
    pool: web::Data<PgPool>,
    path: web::Path<(Uuid, Uuid)>,
    admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let (group_id, member_id) = path.into_inner();
    info!(group_id = %group_id, member_id = %member_id, "Removing carrier group member");

    let result = sqlx::query!(
        "DELETE FROM carrier_group_members WHERE id = $1 AND group_id = $2",
        member_id,
        group_id
    )
    .execute(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Member {} not found in group {}",
            member_id, group_id
        )));
    }

    // Mark group as needing sync
    sqlx::query!(
        "UPDATE carrier_groups SET sync_status = 'pending' WHERE id = $1",
        group_id
    )
    .execute(pool.get_ref())
    .await
    .ok();

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("remove_carrier_group_member")
        .entity_type("carrier_group_member")
        .entity_id(member_id.to_string())
        .details(serde_json::json!({"deleted": true}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message((), "Member removed from group")))
}

// ============================================================================
// Outbound Route Handlers
// ============================================================================

/// List all outbound routes
///
/// GET /api/v1/outbound-routing
#[instrument(skip(pool, user))]
pub async fn list_outbound_routes(
    pool: web::Data<PgPool>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("outbound_routing") {
        return Err(AppError::Forbidden);
    }

    debug!("Listing all outbound routes");

    let routes = sqlx::query!(
        r#"
        SELECT r.id, r.name, r.description, r.prefix,
               r.carrier_group_id, r.carrier_id,
               COALESCE(cg.name, c.name) as "destination_name?",
               CASE WHEN r.carrier_group_id IS NOT NULL THEN 'group' ELSE 'carrier' END as destination_type,
               r.kamailio_ruleid, r.sync_status, r.sync_error,
               r.enabled, r.created_at, r.updated_at
        FROM outbound_routes r
        LEFT JOIN carrier_groups cg ON r.carrier_group_id = cg.id
        LEFT JOIN carriers c ON r.carrier_id = c.id
        ORDER BY LENGTH(r.prefix) DESC, r.prefix
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    let mut enabled_count = 0i64;
    let mut synced_count = 0i64;

    let response: Vec<OutboundRoute> = routes
        .into_iter()
        .map(|r| {
            if r.enabled.unwrap_or(true) {
                enabled_count += 1;
            }
            if r.sync_status.as_deref() == Some("synced") {
                synced_count += 1;
            }

            let priority = route_priority(&r.prefix);

            OutboundRoute {
                id: r.id,
                name: r.name,
                description: r.description,
                prefix: r.prefix,
                carrier_group_id: r.carrier_group_id,
                carrier_id: r.carrier_id,
                destination_name: r.destination_name,
                destination_type: r.destination_type.unwrap_or_else(|| "carrier".to_string()),
                priority,
                kamailio_ruleid: r.kamailio_ruleid,
                sync_status: r
                    .sync_status
                    .as_deref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(SyncStatus::Pending),
                sync_error: r.sync_error,
                enabled: r.enabled.unwrap_or(true),
                created_at: r.created_at.unwrap_or_else(chrono::Utc::now),
                updated_at: r.updated_at.unwrap_or_else(chrono::Utc::now),
            }
        })
        .collect();

    let total = response.len() as i64;

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(OutboundRouteListResponse {
            routes: response,
            total,
            enabled_count,
            synced_count,
        })),
    )
}

/// Get a single outbound route
///
/// GET /api/v1/outbound-routing/{id}
#[instrument(skip(pool, user))]
pub async fn get_outbound_route(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, AppError> {
    if !user.has_module("outbound_routing") {
        return Err(AppError::Forbidden);
    }

    let route_id = path.into_inner();
    debug!(id = %route_id, "Getting outbound route");

    let r = sqlx::query!(
        r#"
        SELECT r.id, r.name, r.description, r.prefix,
               r.carrier_group_id, r.carrier_id,
               COALESCE(cg.name, c.name) as "destination_name?",
               CASE WHEN r.carrier_group_id IS NOT NULL THEN 'group' ELSE 'carrier' END as destination_type,
               r.kamailio_ruleid, r.sync_status, r.sync_error,
               r.enabled, r.created_at, r.updated_at
        FROM outbound_routes r
        LEFT JOIN carrier_groups cg ON r.carrier_group_id = cg.id
        LEFT JOIN carriers c ON r.carrier_id = c.id
        WHERE r.id = $1
        "#,
        route_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Outbound route {} not found", route_id)))?;

    let priority = route_priority(&r.prefix);

    let route = OutboundRoute {
        id: r.id,
        name: r.name,
        description: r.description,
        prefix: r.prefix,
        carrier_group_id: r.carrier_group_id,
        carrier_id: r.carrier_id,
        destination_name: r.destination_name,
        destination_type: r.destination_type.unwrap_or_else(|| "carrier".to_string()),
        priority,
        kamailio_ruleid: r.kamailio_ruleid,
        sync_status: r
            .sync_status
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(SyncStatus::Pending),
        sync_error: r.sync_error,
        enabled: r.enabled.unwrap_or(true),
        created_at: r.created_at.unwrap_or_else(chrono::Utc::now),
        updated_at: r.updated_at.unwrap_or_else(chrono::Utc::now),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(route)))
}

/// Create a new outbound route
///
/// POST /api/v1/outbound-routing
#[instrument(skip(pool, admin, req))]
pub async fn create_outbound_route(
    pool: web::Data<PgPool>,
    admin: AdminUser,
    req: web::Json<CreateOutboundRouteRequest>,
) -> Result<HttpResponse, AppError> {
    info!(name = %req.name, prefix = %req.prefix, "Creating outbound route");

    // Validate prefix
    validate_prefix(&req.prefix)?;

    // Validate destination
    if req.carrier_group_id.is_none() && req.carrier_id.is_none() {
        return Err(AppError::Validation(
            "Either carrier_group_id or carrier_id must be provided".to_string(),
        ));
    }
    if req.carrier_group_id.is_some() && req.carrier_id.is_some() {
        return Err(AppError::Validation(
            "Only one of carrier_group_id or carrier_id should be provided".to_string(),
        ));
    }

    // Check for duplicate prefix
    let existing = sqlx::query!(
        "SELECT id FROM outbound_routes WHERE prefix = $1",
        req.prefix
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    if existing.is_some() {
        return Err(AppError::AlreadyExists(format!(
            "Route with prefix '{}' already exists",
            req.prefix
        )));
    }

    let route_id = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO outbound_routes (
            id, name, description, prefix,
            carrier_group_id, carrier_id, enabled, sync_status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending')
        "#,
        route_id,
        req.name,
        req.description,
        req.prefix,
        req.carrier_group_id,
        req.carrier_id,
        req.enabled.unwrap_or(true),
    )
    .execute(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Sync to Kamailio
    if let Err(e) = sync_outbound_route_to_kamailio(&pool, route_id).await {
        let error_msg = e.to_string();
        let _ = sqlx::query!(
            "UPDATE outbound_routes SET sync_status = 'error', sync_error = $1 WHERE id = $2",
            &error_msg,
            route_id
        )
        .execute(pool.get_ref())
        .await;
        warn!(error = %error_msg, "Failed to sync outbound route to Kamailio");
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("create_outbound_route")
        .entity_type("outbound_route")
        .entity_id(route_id.to_string())
        .details(serde_json::json!({
            "name": req.name,
            "prefix": req.prefix,
        }))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    get_outbound_route(pool, web::Path::from(route_id), admin.0).await
}

/// Update an outbound route
///
/// PUT /api/v1/outbound-routing/{id}
#[instrument(skip(pool, admin, req))]
pub async fn update_outbound_route(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    admin: AdminUser,
    req: web::Json<UpdateOutboundRouteRequest>,
) -> Result<HttpResponse, AppError> {
    let route_id = path.into_inner();
    info!(id = %route_id, "Updating outbound route");

    // Check exists
    let current = sqlx::query!(
        "SELECT carrier_group_id, carrier_id FROM outbound_routes WHERE id = $1",
        route_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Outbound route {} not found", route_id)))?;

    // Validate prefix if provided
    if let Some(ref prefix) = req.prefix {
        validate_prefix(prefix)?;

        // Check for duplicate prefix (excluding self)
        let dup = sqlx::query!(
            "SELECT id FROM outbound_routes WHERE prefix = $1 AND id != $2",
            prefix,
            route_id
        )
        .fetch_optional(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        if dup.is_some() {
            return Err(AppError::AlreadyExists(format!(
                "Route with prefix '{}' already exists",
                prefix
            )));
        }
    }

    // The destination is exclusive: naming one clears the other, otherwise
    // outbound_routes_dest_check rejects the update when switching carrier <-> group
    let (carrier_group_id, carrier_id) = match (req.carrier_group_id, req.carrier_id) {
        (Some(_), Some(_)) => {
            return Err(AppError::Validation(
                "Only one of carrier_group_id or carrier_id should be provided".to_string(),
            ))
        }
        (Some(group_id), None) => (Some(group_id), None),
        (None, Some(carrier_id)) => (None, Some(carrier_id)),
        (None, None) => (current.carrier_group_id, current.carrier_id),
    };

    sqlx::query!(
        r#"
        UPDATE outbound_routes SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            prefix = COALESCE($4, prefix),
            carrier_group_id = $5,
            carrier_id = $6,
            enabled = COALESCE($7, enabled),
            sync_status = 'pending',
            updated_at = NOW()
        WHERE id = $1
        "#,
        route_id,
        req.name,
        req.description,
        req.prefix,
        carrier_group_id,
        carrier_id,
        req.enabled,
    )
    .execute(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Sync to Kamailio
    if let Err(e) = sync_outbound_route_to_kamailio(&pool, route_id).await {
        let error_msg = e.to_string();
        let _ = sqlx::query!(
            "UPDATE outbound_routes SET sync_status = 'error', sync_error = $1 WHERE id = $2",
            &error_msg,
            route_id
        )
        .execute(pool.get_ref())
        .await;
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("update_outbound_route")
        .entity_type("outbound_route")
        .entity_id(route_id.to_string())
        .details(serde_json::json!({"updated": true}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    get_outbound_route(pool, web::Path::from(route_id), admin.0).await
}

/// Delete an outbound route
///
/// DELETE /api/v1/outbound-routing/{id}
#[instrument(skip(pool, admin))]
pub async fn delete_outbound_route(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let route_id = path.into_inner();
    info!(id = %route_id, "Deleting outbound route");

    // Get kamailio_ruleid before deletion
    let route = sqlx::query!(
        "SELECT kamailio_ruleid FROM outbound_routes WHERE id = $1",
        route_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Outbound route {} not found", route_id)))?;

    let result = sqlx::query!("DELETE FROM outbound_routes WHERE id = $1", route_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Outbound route {} not found",
            route_id
        )));
    }

    // Delete from Kamailio if synced
    if let Some(ruleid) = route.kamailio_ruleid {
        if let Err(e) = delete_kamailio_rule(ruleid).await {
            warn!(ruleid = ruleid, error = %e, "Failed to delete rule from Kamailio");
        }
    }

    // Audit log
    if let Ok(audit) = AuditLogBuilder::default()
        .username(admin.username.clone())
        .action("delete_outbound_route")
        .entity_type("outbound_route")
        .entity_id(route_id.to_string())
        .details(serde_json::json!({"deleted": true}))
        .build()
    {
        audit.insert(pool.get_ref()).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        (),
        "Outbound route deleted successfully",
    )))
}

/// Sync all outbound routes to Kamailio
///
/// POST /api/v1/outbound-routing/sync
#[instrument(skip(pool, _admin))]
pub async fn sync_all_outbound_routes(
    pool: web::Data<PgPool>,
    _admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    info!("Syncing all outbound routes to Kamailio");

    // Include disabled routes so their rules get withdrawn from Kamailio
    let routes = sqlx::query!("SELECT id, name FROM outbound_routes")
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    let mut results = Vec::new();
    let mut succeeded = 0;
    let mut failed = 0;

    for route in routes {
        match sync_outbound_route_to_kamailio(&pool, route.id).await {
            Ok(ruleid) => {
                succeeded += 1;
                results.push(OutboundRouteSyncResult {
                    route_id: route.id,
                    route_name: route.name,
                    kamailio_ruleid: ruleid,
                    success: true,
                    error: None,
                });
            }
            Err(e) => {
                failed += 1;
                results.push(OutboundRouteSyncResult {
                    route_id: route.id,
                    route_name: route.name,
                    kamailio_ruleid: None,
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

/// Sync all (carriers, groups, routes) to Kamailio
///
/// POST /api/v1/kamailio/sync-all
#[instrument(skip(pool, _admin))]
pub async fn sync_all_to_kamailio(
    pool: web::Data<PgPool>,
    _admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    info!("Syncing all carriers, groups, and routes to Kamailio");

    let mut carriers_synced = 0;
    let mut carriers_failed = 0;
    let mut groups_synced = 0;
    let mut groups_failed = 0;
    let mut routes_synced = 0;
    let mut routes_failed = 0;
    let mut errors = Vec::new();

    // Sync carriers
    let carriers = sqlx::query!("SELECT id, name FROM carriers WHERE enabled = true")
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    for carrier in carriers {
        match sync_carrier_to_kamailio(&pool, carrier.id).await {
            Ok(_) => carriers_synced += 1,
            Err(e) => {
                carriers_failed += 1;
                errors.push(format!("Carrier '{}': {}", carrier.name, e));
            }
        }
    }

    // Sync carrier groups
    let groups = sqlx::query!("SELECT id, name FROM carrier_groups")
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    for group in groups {
        match sync_carrier_group_to_kamailio(&pool, group.id).await {
            Ok(_) => groups_synced += 1,
            Err(e) => {
                groups_failed += 1;
                errors.push(format!("Group '{}': {}", group.name, e));
            }
        }
    }

    // Sync outbound routes, including the disabled ones so their rules get withdrawn
    let routes = sqlx::query!("SELECT id, name FROM outbound_routes")
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    for route in routes {
        match sync_outbound_route_to_kamailio(&pool, route.id).await {
            Ok(_) => routes_synced += 1,
            Err(e) => {
                routes_failed += 1;
                errors.push(format!("Route '{}': {}", route.name, e));
            }
        }
    }

    // Reload Kamailio
    let kamailio_reloaded = reload_kamailio_routing().await.is_ok();

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(OutboundSyncAllResult {
            carriers_synced,
            carriers_failed,
            groups_synced,
            groups_failed,
            routes_synced,
            routes_failed,
            kamailio_reloaded,
            errors,
        })),
    )
}

/// Reload Kamailio routing tables
///
/// POST /api/v1/kamailio/reload
#[instrument(skip(pool, _admin))]
pub async fn reload_kamailio(
    pool: web::Data<PgPool>,
    _admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    info!("Reloading Kamailio routing tables");

    reload_kamailio_routing().await?;

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        (),
        "Kamailio routing tables reloaded",
    )))
}

// ============================================================================
// Helper Functions
// ============================================================================

/// drouting group holding the outbound rules, matches `FLT_OUTBOUND` in kamailio.cfg
const OUTBOUND_DR_GROUPID: &str = "8000";

/// Validate prefix format
fn validate_prefix(prefix: &str) -> Result<(), AppError> {
    if prefix.is_empty() {
        return Err(AppError::Validation("Prefix cannot be empty".to_string()));
    }

    if !prefix.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::Validation(
            "Prefix must contain only digits".to_string(),
        ));
    }

    Ok(())
}

/// Priority of a rule in dr_rules: drouting evaluates the lowest value first, so a
/// longer prefix must map to a lower number to win over a shorter one.
fn route_priority(prefix: &str) -> i32 {
    (100 - prefix.chars().count() as i32).max(0)
}

/// Sync a single carrier to Kamailio
async fn sync_carrier_to_kamailio(
    pool: &PgPool,
    carrier_id: Uuid,
) -> Result<Option<i32>, AppError> {
    let carrier = sqlx::query!(
        "SELECT id, name, host, port, transport, strip_digits, kamailio_gwid FROM carriers WHERE id = $1",
        carrier_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Carrier {} not found", carrier_id)))?;

    let kamailio_url = std::env::var("KAMAILIO_DATABASE_URL").ok();
    if kamailio_url.is_none() {
        return Ok(None);
    }

    let kamailio_pool = sqlx::mysql::MySqlPool::connect(&kamailio_url.unwrap())
        .await
        .map_err(|e| AppError::Database(format!("Kamailio DB: {}", e)))?;

    let address = gateway_address(&carrier.host, carrier.port);
    let signalling = signalling_from_transport(carrier.transport.as_deref());
    let gwid: i32;

    if let Some(existing_gwid) = carrier.kamailio_gwid {
        sqlx::query(
            "UPDATE dr_gateways SET address = ?, strip = ?, attrs = ?, description = ? WHERE gwid = ?",
        )
        .bind(&address)
        .bind(carrier.strip_digits.unwrap_or(0))
        .bind(gateway_attrs(existing_gwid, GW_TYPE_CARRIER, signalling))
        .bind(&carrier.name)
        .bind(existing_gwid)
        .execute(&kamailio_pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
        gwid = existing_gwid;
    } else {
        let result = sqlx::query(
            "INSERT INTO dr_gateways (type, address, strip, attrs, description) VALUES (?, ?, ?, '', ?)",
        )
        .bind(GW_TYPE_CARRIER)
        .bind(&address)
        .bind(carrier.strip_digits.unwrap_or(0))
        .bind(&carrier.name)
        .execute(&kamailio_pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        gwid = result.last_insert_id() as i32;

        // attrs starts with the gwid, which is only known once the INSERT has assigned it
        sqlx::query("UPDATE dr_gateways SET attrs = ? WHERE gwid = ?")
            .bind(gateway_attrs(gwid, GW_TYPE_CARRIER, signalling))
            .bind(gwid)
            .execute(&kamailio_pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        sqlx::query!(
            "UPDATE carriers SET kamailio_gwid = $1 WHERE id = $2",
            gwid,
            carrier_id
        )
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;
    }

    sqlx::query!(
        "UPDATE carriers SET sync_status = 'synced', sync_error = NULL WHERE id = $1",
        carrier_id
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(Some(gwid))
}

/// Sync a carrier group to Kamailio dr_gw_lists
async fn sync_carrier_group_to_kamailio(
    pool: &PgPool,
    group_id: Uuid,
) -> Result<Option<i32>, AppError> {
    let group = sqlx::query!(
        "SELECT id, name, kamailio_group_id FROM carrier_groups WHERE id = $1",
        group_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Carrier group {} not found", group_id)))?;

    // Get members with their kamailio_gwids
    let members = sqlx::query!(
        r#"
        SELECT c.kamailio_gwid
        FROM carrier_group_members m
        JOIN carriers c ON m.carrier_id = c.id
        WHERE m.group_id = $1
        ORDER BY m.priority, m.weight DESC
        "#,
        group_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Build gwlist string (comma-separated gwids)
    let gwlist: String = members
        .iter()
        .filter_map(|m| m.kamailio_gwid)
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    if gwlist.is_empty() {
        return Ok(None);
    }

    let kamailio_url = std::env::var("KAMAILIO_DATABASE_URL").ok();
    if kamailio_url.is_none() {
        return Ok(None);
    }

    let kamailio_pool = sqlx::mysql::MySqlPool::connect(&kamailio_url.unwrap())
        .await
        .map_err(|e| AppError::Database(format!("Kamailio DB: {}", e)))?;

    let kam_id: i32;

    if let Some(existing_id) = group.kamailio_group_id {
        sqlx::query("UPDATE dr_gw_lists SET gwlist = ?, description = ? WHERE id = ?")
            .bind(&gwlist)
            .bind(&group.name)
            .bind(existing_id)
            .execute(&kamailio_pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        kam_id = existing_id;
    } else {
        let result = sqlx::query("INSERT INTO dr_gw_lists (gwlist, description) VALUES (?, ?)")
            .bind(&gwlist)
            .bind(&group.name)
            .execute(&kamailio_pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        kam_id = result.last_insert_id() as i32;
        sqlx::query!(
            "UPDATE carrier_groups SET kamailio_group_id = $1 WHERE id = $2",
            kam_id,
            group_id
        )
        .execute(pool)
        .await
        .ok();
    }

    sqlx::query!(
        "UPDATE carrier_groups SET sync_status = 'synced', sync_error = NULL WHERE id = $1",
        group_id
    )
    .execute(pool)
    .await
    .ok();

    Ok(Some(kam_id))
}

/// Sync an outbound route to Kamailio dr_rules
async fn sync_outbound_route_to_kamailio(
    pool: &PgPool,
    route_id: Uuid,
) -> Result<Option<i32>, AppError> {
    let route = sqlx::query!(
        r#"
        SELECT r.id, r.name, r.prefix, r.enabled, r.kamailio_ruleid,
               cg.kamailio_group_id, c.kamailio_gwid,
               COALESCE(cg.name, c.name) as "destination_name?"
        FROM outbound_routes r
        LEFT JOIN carrier_groups cg ON r.carrier_group_id = cg.id
        LEFT JOIN carriers c ON r.carrier_id = c.id
        WHERE r.id = $1
        "#,
        route_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?
    .ok_or_else(|| AppError::NotFound(format!("Outbound route {} not found", route_id)))?;

    // A disabled route must not keep matching calls in Kamailio
    if !route.enabled.unwrap_or(true) {
        if let Some(ruleid) = route.kamailio_ruleid {
            delete_kamailio_rule(ruleid).await?;
        }

        sqlx::query!(
            "UPDATE outbound_routes \
             SET kamailio_ruleid = NULL, sync_status = 'synced', sync_error = NULL \
             WHERE id = $1",
            route_id
        )
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        return Ok(None);
    }

    // Determine gwlist (either from group or single carrier)
    let gwlist = if let Some(group_id) = route.kamailio_group_id {
        format!("#{}", group_id)
    } else if let Some(gwid) = route.kamailio_gwid {
        gwid.to_string()
    } else {
        return Err(AppError::Validation(format!(
            "Destination '{}' is not synced to Kamailio yet, sync carriers and carrier groups first",
            route.destination_name.unwrap_or_else(|| "unknown".to_string())
        )));
    };

    let Some(kamailio_url) = std::env::var("KAMAILIO_DATABASE_URL").ok() else {
        warn!(
            route_id = %route_id,
            "KAMAILIO_DATABASE_URL not set, skipping Kamailio sync of outbound route"
        );
        return Ok(None);
    };

    let kamailio_pool = sqlx::mysql::MySqlPool::connect(&kamailio_url)
        .await
        .map_err(|e| AppError::Database(format!("Kamailio DB: {}", e)))?;

    let priority = route_priority(&route.prefix);
    let description = format!("name:{}", route.name);

    let ruleid: i32 = match route.kamailio_ruleid {
        Some(existing_id) => existing_id,
        None => {
            // A rule for this prefix may already exist in dr_rules, e.g. created from the
            // dsiprouter GUI. Adopt it instead of inserting a second rule for the same prefix.
            let adopted: Option<i64> = sqlx::query_scalar(
                "SELECT CAST(ruleid AS SIGNED) FROM dr_rules \
                 WHERE groupid = ? AND prefix = ? ORDER BY ruleid LIMIT 1",
            )
            .bind(OUTBOUND_DR_GROUPID)
            .bind(&route.prefix)
            .fetch_optional(&kamailio_pool)
            .await
            .map_err(|e| AppError::Database(format!("Kamailio DB: {}", e)))?;

            if let Some(adopted_id) = adopted {
                warn!(
                    route_id = %route_id,
                    ruleid = adopted_id,
                    prefix = %route.prefix,
                    "Prefix already had a dr_rules entry, adopting it instead of creating a duplicate"
                );
                adopted_id as i32
            } else {
                // timerec and routeid are NOT NULL without a default, and description follows the
                // 'name:<route>' convention the dsiprouter GUI parses
                let result = sqlx::query(
                    "INSERT INTO dr_rules (groupid, prefix, timerec, routeid, gwlist, priority, description) \
                     VALUES (?, ?, '', '', ?, ?, ?)",
                )
                .bind(OUTBOUND_DR_GROUPID)
                .bind(&route.prefix)
                .bind(&gwlist)
                .bind(priority)
                .bind(&description)
                .execute(&kamailio_pool)
                .await
                .map_err(|e| AppError::Database(format!("Kamailio DB: {}", e)))?;

                let new_id = result.last_insert_id() as i32;

                sqlx::query!(
                    "UPDATE outbound_routes \
                     SET kamailio_ruleid = $1, sync_status = 'synced', sync_error = NULL \
                     WHERE id = $2",
                    new_id,
                    route_id
                )
                .execute(pool)
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;

                return Ok(Some(new_id));
            }
        }
    };

    sqlx::query(
        "UPDATE dr_rules SET prefix = ?, gwlist = ?, priority = ?, description = ? WHERE ruleid = ?",
    )
    .bind(&route.prefix)
    .bind(&gwlist)
    .bind(priority)
    .bind(&description)
    .bind(ruleid)
    .execute(&kamailio_pool)
    .await
    .map_err(|e| AppError::Database(format!("Kamailio DB: {}", e)))?;

    sqlx::query!(
        "UPDATE outbound_routes \
         SET kamailio_ruleid = $1, sync_status = 'synced', sync_error = NULL \
         WHERE id = $2",
        ruleid,
        route_id
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(Some(ruleid))
}

/// Delete a group from Kamailio
async fn delete_kamailio_group(kam_id: i32) -> Result<(), AppError> {
    let kamailio_url = std::env::var("KAMAILIO_DATABASE_URL").ok();
    if kamailio_url.is_none() {
        return Ok(());
    }

    let kamailio_pool = sqlx::mysql::MySqlPool::connect(&kamailio_url.unwrap())
        .await
        .map_err(|e| AppError::Database(format!("Kamailio DB: {}", e)))?;

    sqlx::query("DELETE FROM dr_gw_lists WHERE id = ?")
        .bind(kam_id)
        .execute(&kamailio_pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(())
}

/// Delete a rule from Kamailio
async fn delete_kamailio_rule(ruleid: i32) -> Result<(), AppError> {
    let kamailio_url = std::env::var("KAMAILIO_DATABASE_URL").ok();
    if kamailio_url.is_none() {
        return Ok(());
    }

    let kamailio_pool = sqlx::mysql::MySqlPool::connect(&kamailio_url.unwrap())
        .await
        .map_err(|e| AppError::Database(format!("Kamailio DB: {}", e)))?;

    sqlx::query("DELETE FROM dr_rules WHERE ruleid = ?")
        .bind(ruleid)
        .execute(&kamailio_pool)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

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
        return Err(AppError::Internal(format!(
            "kamcmd drouting.reload failed: {}",
            stderr.trim()
        )));
    }

    Ok(())
}

// ============================================================================
// Route Configuration
// ============================================================================

/// Configure outbound routing routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/carrier-groups")
            .route("", web::get().to(list_carrier_groups))
            .route("", web::post().to(create_carrier_group))
            .route("/{id}", web::get().to(get_carrier_group))
            .route("/{id}", web::put().to(update_carrier_group))
            .route("/{id}", web::delete().to(delete_carrier_group))
            .route("/{id}/members", web::post().to(add_carrier_group_member))
            .route(
                "/{group_id}/members/{member_id}",
                web::put().to(update_carrier_group_member),
            )
            .route(
                "/{group_id}/members/{member_id}",
                web::delete().to(remove_carrier_group_member),
            ),
    )
    .service(
        web::scope("/outbound-routing")
            .route("", web::get().to(list_outbound_routes))
            .route("", web::post().to(create_outbound_route))
            .route("/sync", web::post().to(sync_all_outbound_routes))
            .route("/{id}", web::get().to(get_outbound_route))
            .route("/{id}", web::put().to(update_outbound_route))
            .route("/{id}", web::delete().to(delete_outbound_route)),
    )
    .service(
        web::scope("/kamailio")
            .route("/sync-all", web::post().to(sync_all_to_kamailio))
            .route("/reload", web::post().to(reload_kamailio)),
    );
}
