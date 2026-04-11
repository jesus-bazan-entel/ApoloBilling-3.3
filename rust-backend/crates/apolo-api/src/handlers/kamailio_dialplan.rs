//! Kamailio Dialplan handlers
//!
//! HTTP handlers for Kamailio outbound routing management via MySQL.
//! Manages dr_gw_lists (carrier groups), dr_gateways (carriers), and dr_rules (outbound routes).

use crate::dto::ApiResponse;
use actix_web::{web, HttpResponse};
use apolo_auth::SuperadminUser;
use apolo_core::AppError;
use serde::{Deserialize, Serialize};
use serde_json;
use sqlx::mysql::MySqlPool;
use tracing::{error, info, instrument};

// ============================================================================
// DTOs
// ============================================================================

/// Carrier Group (dr_gw_lists table)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CarrierGroup {
    pub id: u32,
    pub gwlist: String,
    pub description: String,
}

/// Request to create/update a carrier group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarrierGroupRequest {
    pub description: String,
    pub gwlist: Option<String>,
}

/// Carrier/Gateway (dr_gateways table)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Carrier {
    pub gwid: u32,
    #[sqlx(rename = "type")]
    pub gw_type: u32,
    pub address: String,
    pub strip: u32,
    pub pri_prefix: String,
    pub attrs: String,
    pub description: String,
}

/// Request to create/update a carrier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarrierRequest {
    pub address: String,
    pub description: String,
    #[serde(default)]
    pub strip: u32,
    #[serde(default)]
    pub pri_prefix: String,
    #[serde(default)]
    pub attrs: String,
    #[serde(default = "default_gw_type")]
    pub gw_type: u32,
}

fn default_gw_type() -> u32 {
    1
}

/// Outbound Route (dr_rules table)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OutboundRoute {
    pub ruleid: u32,
    pub groupid: String,
    pub prefix: String,
    pub timerec: String,
    pub priority: i32,
    pub routeid: String,
    pub gwlist: String,
    pub description: String,
}

/// Request to create/update an outbound route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundRouteRequest {
    #[serde(default)]
    pub groupid: String,
    pub prefix: String,
    #[serde(default)]
    pub timerec: String,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(default)]
    pub routeid: String,
    pub gwlist: String,
    #[serde(default)]
    pub description: String,
}

fn default_priority() -> i32 {
    1
}

/// Carrier group with associated carriers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarrierGroupWithCarriers {
    #[serde(flatten)]
    pub group: CarrierGroup,
    pub carriers: Vec<Carrier>,
}

// ============================================================================
// Carrier Group Handlers
// ============================================================================

/// List all carrier groups
///
/// GET /api/v1/kamailio-dialplan/groups
#[instrument(skip(pool, _admin))]
pub async fn list_carrier_groups(
    pool: web::Data<MySqlPool>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!("Listing Kamailio carrier groups");

    let groups: Vec<CarrierGroup> = sqlx::query_as(
        "SELECT id, gwlist, description FROM dr_gw_lists ORDER BY id"
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to fetch carrier groups");
        AppError::Database(format!("Failed to fetch carrier groups: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(groups)))
}

/// Get a single carrier group with its carriers
///
/// GET /api/v1/kamailio-dialplan/groups/{id}
#[instrument(skip(pool, _admin))]
pub async fn get_carrier_group(
    pool: web::Data<MySqlPool>,
    path: web::Path<u32>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let group_id = path.into_inner();
    info!(group_id = group_id, "Getting carrier group");

    let group: Option<CarrierGroup> = sqlx::query_as(
        "SELECT id, gwlist, description FROM dr_gw_lists WHERE id = ?"
    )
    .bind(group_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to fetch carrier group");
        AppError::Database(format!("Failed to fetch carrier group: {}", e))
    })?;

    let group = group.ok_or_else(|| AppError::NotFound(format!("Carrier group {} not found", group_id)))?;

    // Parse gwlist to get carrier IDs
    let carrier_ids: Vec<i32> = group.gwlist
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let carriers: Vec<Carrier> = if carrier_ids.is_empty() {
        Vec::new()
    } else {
        let placeholders = carrier_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT gwid, type, address, strip, pri_prefix, attrs, description FROM dr_gateways WHERE gwid IN ({}) ORDER BY gwid",
            placeholders
        );
        let mut q = sqlx::query_as::<_, Carrier>(&query);
        for id in &carrier_ids {
            q = q.bind(id);
        }
        q.fetch_all(pool.get_ref())
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to fetch carriers for group");
                AppError::Database(format!("Failed to fetch carriers: {}", e))
            })?
    };

    let result = CarrierGroupWithCarriers { group, carriers };
    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

/// Create a new carrier group
///
/// POST /api/v1/kamailio-dialplan/groups
#[instrument(skip(pool, _admin))]
pub async fn create_carrier_group(
    pool: web::Data<MySqlPool>,
    req: web::Json<CarrierGroupRequest>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!(description = %req.description, "Creating carrier group");

    if req.description.trim().is_empty() {
        return Err(AppError::Validation("Description cannot be empty".to_string()));
    }

    let gwlist = req.gwlist.clone().unwrap_or_default();

    let result = sqlx::query(
        "INSERT INTO dr_gw_lists (gwlist, description) VALUES (?, ?)"
    )
    .bind(&gwlist)
    .bind(&req.description)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to create carrier group");
        AppError::Database(format!("Failed to create carrier group: {}", e))
    })?;

    let group_id = result.last_insert_id() as u32;
    let group = CarrierGroup {
        id: group_id,
        gwlist,
        description: req.description.clone(),
    };

    Ok(HttpResponse::Created().json(ApiResponse::success(group)))
}

/// Update a carrier group
///
/// PUT /api/v1/kamailio-dialplan/groups/{id}
#[instrument(skip(pool, _admin))]
pub async fn update_carrier_group(
    pool: web::Data<MySqlPool>,
    path: web::Path<u32>,
    req: web::Json<CarrierGroupRequest>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let group_id = path.into_inner();
    info!(group_id = group_id, "Updating carrier group");

    if req.description.trim().is_empty() {
        return Err(AppError::Validation("Description cannot be empty".to_string()));
    }

    // Build update query
    let gwlist = req.gwlist.clone().unwrap_or_default();

    let result = sqlx::query(
        "UPDATE dr_gw_lists SET gwlist = ?, description = ? WHERE id = ?"
    )
    .bind(&gwlist)
    .bind(&req.description)
    .bind(group_id)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to update carrier group");
        AppError::Database(format!("Failed to update carrier group: {}", e))
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Carrier group {} not found", group_id)));
    }

    let group = CarrierGroup {
        id: group_id,
        gwlist,
        description: req.description.clone(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(group)))
}

/// Delete a carrier group
///
/// DELETE /api/v1/kamailio-dialplan/groups/{id}
#[instrument(skip(pool, _admin))]
pub async fn delete_carrier_group(
    pool: web::Data<MySqlPool>,
    path: web::Path<u32>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let group_id = path.into_inner();
    info!(group_id = group_id, "Deleting carrier group");

    let result = sqlx::query("DELETE FROM dr_gw_lists WHERE id = ?")
        .bind(group_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to delete carrier group");
            AppError::Database(format!("Failed to delete carrier group: {}", e))
        })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Carrier group {} not found", group_id)));
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        serde_json::json!({ "deleted": group_id }),
        "Carrier group deleted",
    )))
}

// ============================================================================
// Carrier/Gateway Handlers
// ============================================================================

/// List all carriers/gateways
///
/// GET /api/v1/kamailio-dialplan/carriers
#[instrument(skip(pool, _admin))]
pub async fn list_carriers(
    pool: web::Data<MySqlPool>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!("Listing Kamailio carriers");

    let carriers: Vec<Carrier> = sqlx::query_as(
        "SELECT gwid, type, address, strip, pri_prefix, attrs, description FROM dr_gateways ORDER BY gwid"
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to fetch carriers");
        AppError::Database(format!("Failed to fetch carriers: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(carriers)))
}

/// Get a single carrier
///
/// GET /api/v1/kamailio-dialplan/carriers/{id}
#[instrument(skip(pool, _admin))]
pub async fn get_carrier(
    pool: web::Data<MySqlPool>,
    path: web::Path<u32>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let carrier_id = path.into_inner();
    info!(carrier_id = carrier_id, "Getting carrier");

    let carrier: Option<Carrier> = sqlx::query_as(
        "SELECT gwid, type, address, strip, pri_prefix, attrs, description FROM dr_gateways WHERE gwid = ?"
    )
    .bind(carrier_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to fetch carrier");
        AppError::Database(format!("Failed to fetch carrier: {}", e))
    })?;

    let carrier = carrier.ok_or_else(|| AppError::NotFound(format!("Carrier {} not found", carrier_id)))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(carrier)))
}

/// Create a new carrier
///
/// POST /api/v1/kamailio-dialplan/carriers
#[instrument(skip(pool, _admin))]
pub async fn create_carrier(
    pool: web::Data<MySqlPool>,
    req: web::Json<CarrierRequest>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!(address = %req.address, "Creating carrier");

    if req.address.trim().is_empty() {
        return Err(AppError::Validation("Address cannot be empty".to_string()));
    }

    let result = sqlx::query(
        "INSERT INTO dr_gateways (type, address, strip, pri_prefix, attrs, description) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(req.gw_type)
    .bind(&req.address)
    .bind(req.strip)
    .bind(&req.pri_prefix)
    .bind(&req.attrs)
    .bind(&req.description)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to create carrier");
        AppError::Database(format!("Failed to create carrier: {}", e))
    })?;

    let carrier_id = result.last_insert_id() as u32;
    let carrier = Carrier {
        gwid: carrier_id,
        gw_type: req.gw_type,
        address: req.address.clone(),
        strip: req.strip,
        pri_prefix: req.pri_prefix.clone(),
        attrs: req.attrs.clone(),
        description: req.description.clone(),
    };

    Ok(HttpResponse::Created().json(ApiResponse::success(carrier)))
}

/// Update a carrier
///
/// PUT /api/v1/kamailio-dialplan/carriers/{id}
#[instrument(skip(pool, _admin))]
pub async fn update_carrier(
    pool: web::Data<MySqlPool>,
    path: web::Path<u32>,
    req: web::Json<CarrierRequest>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let carrier_id = path.into_inner();
    info!(carrier_id = carrier_id, "Updating carrier");

    if req.address.trim().is_empty() {
        return Err(AppError::Validation("Address cannot be empty".to_string()));
    }

    let result = sqlx::query(
        "UPDATE dr_gateways SET type = ?, address = ?, strip = ?, pri_prefix = ?, attrs = ?, description = ? WHERE gwid = ?"
    )
    .bind(req.gw_type)
    .bind(&req.address)
    .bind(req.strip)
    .bind(&req.pri_prefix)
    .bind(&req.attrs)
    .bind(&req.description)
    .bind(carrier_id)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to update carrier");
        AppError::Database(format!("Failed to update carrier: {}", e))
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Carrier {} not found", carrier_id)));
    }

    let carrier = Carrier {
        gwid: carrier_id,
        gw_type: req.gw_type,
        address: req.address.clone(),
        strip: req.strip,
        pri_prefix: req.pri_prefix.clone(),
        attrs: req.attrs.clone(),
        description: req.description.clone(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(carrier)))
}

/// Delete a carrier
///
/// DELETE /api/v1/kamailio-dialplan/carriers/{id}
#[instrument(skip(pool, _admin))]
pub async fn delete_carrier(
    pool: web::Data<MySqlPool>,
    path: web::Path<u32>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let carrier_id = path.into_inner();
    info!(carrier_id = carrier_id, "Deleting carrier");

    let result = sqlx::query("DELETE FROM dr_gateways WHERE gwid = ?")
        .bind(carrier_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to delete carrier");
            AppError::Database(format!("Failed to delete carrier: {}", e))
        })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Carrier {} not found", carrier_id)));
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        serde_json::json!({ "deleted": carrier_id }),
        "Carrier deleted",
    )))
}

// ============================================================================
// Outbound Route Handlers
// ============================================================================

/// List all outbound routes
///
/// GET /api/v1/kamailio-dialplan/routes
#[instrument(skip(pool, _admin))]
pub async fn list_outbound_routes(
    pool: web::Data<MySqlPool>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!("Listing Kamailio outbound routes");

    let routes: Vec<OutboundRoute> = sqlx::query_as(
        "SELECT ruleid, groupid, prefix, timerec, priority, routeid, gwlist, description FROM dr_rules ORDER BY prefix, priority"
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to fetch outbound routes");
        AppError::Database(format!("Failed to fetch outbound routes: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(routes)))
}

/// Get a single outbound route
///
/// GET /api/v1/kamailio-dialplan/routes/{id}
#[instrument(skip(pool, _admin))]
pub async fn get_outbound_route(
    pool: web::Data<MySqlPool>,
    path: web::Path<u32>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let route_id = path.into_inner();
    info!(route_id = route_id, "Getting outbound route");

    let route: Option<OutboundRoute> = sqlx::query_as(
        "SELECT ruleid, groupid, prefix, timerec, priority, routeid, gwlist, description FROM dr_rules WHERE ruleid = ?"
    )
    .bind(route_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to fetch outbound route");
        AppError::Database(format!("Failed to fetch outbound route: {}", e))
    })?;

    let route = route.ok_or_else(|| AppError::NotFound(format!("Outbound route {} not found", route_id)))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(route)))
}

/// Create a new outbound route
///
/// POST /api/v1/kamailio-dialplan/routes
#[instrument(skip(pool, _admin))]
pub async fn create_outbound_route(
    pool: web::Data<MySqlPool>,
    req: web::Json<OutboundRouteRequest>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!(prefix = %req.prefix, "Creating outbound route");

    if req.gwlist.trim().is_empty() {
        return Err(AppError::Validation("Gateway list cannot be empty".to_string()));
    }

    let result = sqlx::query(
        "INSERT INTO dr_rules (groupid, prefix, timerec, priority, routeid, gwlist, description) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&req.groupid)
    .bind(&req.prefix)
    .bind(&req.timerec)
    .bind(req.priority)
    .bind(&req.routeid)
    .bind(&req.gwlist)
    .bind(&req.description)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to create outbound route");
        AppError::Database(format!("Failed to create outbound route: {}", e))
    })?;

    let route_id = result.last_insert_id() as u32;
    let route = OutboundRoute {
        ruleid: route_id,
        groupid: req.groupid.clone(),
        prefix: req.prefix.clone(),
        timerec: req.timerec.clone(),
        priority: req.priority,
        routeid: req.routeid.clone(),
        gwlist: req.gwlist.clone(),
        description: req.description.clone(),
    };

    Ok(HttpResponse::Created().json(ApiResponse::success(route)))
}

/// Update an outbound route
///
/// PUT /api/v1/kamailio-dialplan/routes/{id}
#[instrument(skip(pool, _admin))]
pub async fn update_outbound_route(
    pool: web::Data<MySqlPool>,
    path: web::Path<u32>,
    req: web::Json<OutboundRouteRequest>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let route_id = path.into_inner();
    info!(route_id = route_id, "Updating outbound route");

    if req.gwlist.trim().is_empty() {
        return Err(AppError::Validation("Gateway list cannot be empty".to_string()));
    }

    let result = sqlx::query(
        "UPDATE dr_rules SET groupid = ?, prefix = ?, timerec = ?, priority = ?, routeid = ?, gwlist = ?, description = ? WHERE ruleid = ?"
    )
    .bind(&req.groupid)
    .bind(&req.prefix)
    .bind(&req.timerec)
    .bind(req.priority)
    .bind(&req.routeid)
    .bind(&req.gwlist)
    .bind(&req.description)
    .bind(route_id)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to update outbound route");
        AppError::Database(format!("Failed to update outbound route: {}", e))
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Outbound route {} not found", route_id)));
    }

    let route = OutboundRoute {
        ruleid: route_id,
        groupid: req.groupid.clone(),
        prefix: req.prefix.clone(),
        timerec: req.timerec.clone(),
        priority: req.priority,
        routeid: req.routeid.clone(),
        gwlist: req.gwlist.clone(),
        description: req.description.clone(),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(route)))
}

/// Delete an outbound route
///
/// DELETE /api/v1/kamailio-dialplan/routes/{id}
#[instrument(skip(pool, _admin))]
pub async fn delete_outbound_route(
    pool: web::Data<MySqlPool>,
    path: web::Path<u32>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    let route_id = path.into_inner();
    info!(route_id = route_id, "Deleting outbound route");

    let result = sqlx::query("DELETE FROM dr_rules WHERE ruleid = ?")
        .bind(route_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to delete outbound route");
            AppError::Database(format!("Failed to delete outbound route: {}", e))
        })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Outbound route {} not found", route_id)));
    }

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        serde_json::json!({ "deleted": route_id }),
        "Outbound route deleted",
    )))
}

/// Reload Kamailio routing tables
///
/// POST /api/v1/kamailio-dialplan/reload
#[instrument(skip(pool, _admin))]
pub async fn reload_kamailio_routes(
    pool: web::Data<MySqlPool>,
    _admin: SuperadminUser,
) -> Result<HttpResponse, AppError> {
    info!("Reloading Kamailio routing tables");

    // Use kamcmd to reload drouting tables
    use tokio::process::Command;

    let output = Command::new("kamcmd")
        .args(["drouting.reload"])
        .output()
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to execute kamcmd");
            AppError::Internal(format!("Failed to reload routes: {}", e))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!(stderr = %stderr, "kamcmd drouting.reload failed");
        return Err(AppError::Internal(format!("Failed to reload routes: {}", stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    info!(stdout = %stdout, "Kamailio routes reloaded successfully");

    Ok(HttpResponse::Ok().json(ApiResponse::with_message(
        serde_json::json!({ "reloaded": true }),
        "Kamailio routing tables reloaded",
    )))
}

/// Configure Kamailio dialplan routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/kamailio-dialplan")
            // Reload routes
            .route("/reload", web::post().to(reload_kamailio_routes))
            // Carrier groups
            .route("/groups", web::get().to(list_carrier_groups))
            .route("/groups", web::post().to(create_carrier_group))
            .route("/groups/{id}", web::get().to(get_carrier_group))
            .route("/groups/{id}", web::put().to(update_carrier_group))
            .route("/groups/{id}", web::delete().to(delete_carrier_group))
            // Carriers/Gateways
            .route("/carriers", web::get().to(list_carriers))
            .route("/carriers", web::post().to(create_carrier))
            .route("/carriers/{id}", web::get().to(get_carrier))
            .route("/carriers/{id}", web::put().to(update_carrier))
            .route("/carriers/{id}", web::delete().to(delete_carrier))
            // Outbound routes
            .route("/routes", web::get().to(list_outbound_routes))
            .route("/routes", web::post().to(create_outbound_route))
            .route("/routes/{id}", web::get().to(get_outbound_route))
            .route("/routes/{id}", web::put().to(update_outbound_route))
            .route("/routes/{id}", web::delete().to(delete_outbound_route)),
    );
}
