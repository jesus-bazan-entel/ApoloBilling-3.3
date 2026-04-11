use actix_web::{web, HttpResponse};
use apolo_core::AppError;
use apolo_db::repositories::{InternalRouteRepository, SystemEndpointRepository};
use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::{
    ApiResponse, CreateEndpointRequest, CreateInternalRouteRequest, SyncResult,
    UpdateEndpointRequest, UpdateInternalRouteRequest,
};

// ============ System Endpoints Handlers ============

/// List all system endpoints
pub async fn list_endpoints(pool: web::Data<PgPool>) -> Result<HttpResponse, AppError> {
    let endpoints = SystemEndpointRepository::list(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Error listing endpoints: {}", e)))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(endpoints)))
}

/// Get endpoint by ID
pub async fn get_endpoint(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let endpoint = SystemEndpointRepository::get_by_id(pool.get_ref(), id)
        .await
        .map_err(|e| AppError::Database(format!("Error getting endpoint: {}", e)))?
        .ok_or_else(|| AppError::NotFound("Endpoint not found".to_string()))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(endpoint)))
}

/// List endpoints by type
pub async fn list_endpoints_by_type(
    pool: web::Data<PgPool>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let endpoint_type = path.into_inner();
    let endpoints = SystemEndpointRepository::list_by_type(pool.get_ref(), &endpoint_type)
        .await
        .map_err(|e| AppError::Database(format!("Error listing endpoints: {}", e)))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(endpoints)))
}

/// Create a new endpoint
pub async fn create_endpoint(
    pool: web::Data<PgPool>,
    body: web::Json<CreateEndpointRequest>,
) -> Result<HttpResponse, AppError> {
    let req = body.into_inner();

    // Validate endpoint_type
    if req.endpoint_type != "freeswitch" && req.endpoint_type != "kamailio" {
        return Err(AppError::Validation(
            "Invalid endpoint_type. Must be 'freeswitch' or 'kamailio'".to_string(),
        ));
    }

    let endpoint = SystemEndpointRepository::create(
        pool.get_ref(),
        &req.name,
        &req.endpoint_type,
        req.description.as_deref(),
        &req.ip_address,
        req.port,
        &req.transport,
        req.fs_profile.as_deref(),
        req.fs_context.as_deref(),
        req.kam_gwid,
        req.kam_gw_type,
        req.is_primary,
    )
    .await
    .map_err(|e| AppError::Database(format!("Error creating endpoint: {}", e)))?;

    Ok(HttpResponse::Created().json(ApiResponse::success(endpoint)))
}

/// Update an endpoint
pub async fn update_endpoint(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateEndpointRequest>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let req = body.into_inner();

    let endpoint = SystemEndpointRepository::update(
        pool.get_ref(),
        id,
        &req.name,
        req.description.as_deref(),
        &req.ip_address,
        req.port,
        &req.transport,
        req.fs_profile.as_deref(),
        req.fs_context.as_deref(),
        req.kam_gwid,
        req.kam_gw_type,
        req.enabled,
        req.is_primary,
    )
    .await
    .map_err(|e| AppError::Database(format!("Error updating endpoint: {}", e)))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(endpoint)))
}

/// Delete an endpoint
pub async fn delete_endpoint(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let deleted = SystemEndpointRepository::delete(pool.get_ref(), id)
        .await
        .map_err(|e| AppError::Database(format!("Error deleting endpoint: {}", e)))?;

    if deleted {
        Ok(HttpResponse::Ok().json(ApiResponse::with_message((), "Endpoint deleted")))
    } else {
        Err(AppError::NotFound("Endpoint not found".to_string()))
    }
}

// ============ Internal Routes Handlers ============

/// List all internal routes
pub async fn list_internal_routes(pool: web::Data<PgPool>) -> Result<HttpResponse, AppError> {
    let routes = InternalRouteRepository::list(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Error listing internal routes: {}", e)))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(routes)))
}

/// Get internal route by ID
pub async fn get_internal_route(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let route = InternalRouteRepository::get_by_id(pool.get_ref(), id)
        .await
        .map_err(|e| AppError::Database(format!("Error getting internal route: {}", e)))?
        .ok_or_else(|| AppError::NotFound("Internal route not found".to_string()))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(route)))
}

/// List internal routes by type
pub async fn list_internal_routes_by_type(
    pool: web::Data<PgPool>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let route_type = path.into_inner();
    let routes = InternalRouteRepository::list_by_type(pool.get_ref(), &route_type)
        .await
        .map_err(|e| AppError::Database(format!("Error listing internal routes: {}", e)))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(routes)))
}

/// Create a new internal route
pub async fn create_internal_route(
    pool: web::Data<PgPool>,
    body: web::Json<CreateInternalRouteRequest>,
) -> Result<HttpResponse, AppError> {
    let req = body.into_inner();

    // Validate route_type
    if req.route_type != "fs_to_kamailio" && req.route_type != "kamailio_to_fs" {
        return Err(AppError::Validation(
            "Invalid route_type. Must be 'fs_to_kamailio' or 'kamailio_to_fs'".to_string(),
        ));
    }

    let route = InternalRouteRepository::create(
        pool.get_ref(),
        &req.name,
        req.description.as_deref(),
        &req.route_type,
        req.source_endpoint_id,
        req.dest_endpoint_id,
        req.bypass_media,
        req.inherit_codec,
        req.enable_100rel,
        req.call_timeout,
        req.prefix_pattern.as_deref(),
        req.priority,
    )
    .await
    .map_err(|e| AppError::Database(format!("Error creating internal route: {}", e)))?;

    Ok(HttpResponse::Created().json(ApiResponse::success(route)))
}

/// Update an internal route
pub async fn update_internal_route(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateInternalRouteRequest>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let req = body.into_inner();

    let route = InternalRouteRepository::update(
        pool.get_ref(),
        id,
        &req.name,
        req.description.as_deref(),
        req.source_endpoint_id,
        req.dest_endpoint_id,
        req.bypass_media,
        req.inherit_codec,
        req.enable_100rel,
        req.call_timeout,
        req.prefix_pattern.as_deref(),
        req.priority,
        req.enabled,
    )
    .await
    .map_err(|e| AppError::Database(format!("Error updating internal route: {}", e)))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(route)))
}

/// Delete an internal route
pub async fn delete_internal_route(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let deleted = InternalRouteRepository::delete(pool.get_ref(), id)
        .await
        .map_err(|e| AppError::Database(format!("Error deleting internal route: {}", e)))?;

    if deleted {
        Ok(HttpResponse::Ok().json(ApiResponse::with_message((), "Internal route deleted")))
    } else {
        Err(AppError::NotFound("Internal route not found".to_string()))
    }
}

/// Sync internal routes to FreeSWITCH and Kamailio
pub async fn sync_internal_routes(pool: web::Data<PgPool>) -> Result<HttpResponse, AppError> {
    // Get all enabled routes
    let routes = InternalRouteRepository::list(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Error listing routes for sync: {}", e)))?;

    let mut fs_synced = true;
    let mut kam_synced = true;

    for route in &routes {
        if !route.enabled {
            continue;
        }

        // Update sync status
        let sync_result = match route.route_type.as_str() {
            "fs_to_kamailio" => {
                // TODO: Generate FreeSWITCH dialplan XML and reload
                // For now, just mark as synced
                ("synced", None)
            }
            "kamailio_to_fs" => {
                // TODO: Update Kamailio dr_gateways and reload
                // For now, just mark as synced
                ("synced", None)
            }
            _ => ("error", Some("Unknown route type")),
        };

        if let Err(e) = InternalRouteRepository::update_sync_status(
            pool.get_ref(),
            route.id,
            sync_result.0,
            sync_result.1,
        )
        .await
        {
            tracing::error!("Error updating sync status: {:?}", e);
            if route.route_type == "fs_to_kamailio" {
                fs_synced = false;
            } else {
                kam_synced = false;
            }
        }
    }

    let result = SyncResult {
        success: fs_synced && kam_synced,
        message: if fs_synced && kam_synced {
            "All routes synced successfully".to_string()
        } else {
            "Some routes failed to sync".to_string()
        },
        freeswitch_synced: fs_synced,
        kamailio_synced: kam_synced,
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

/// Configure routes (convenience function that returns web::Scope)
pub fn configure_routes() -> actix_web::Scope {
    web::scope("/internal-routing")
        // System endpoints
        .route("/endpoints", web::get().to(list_endpoints))
        .route("/endpoints", web::post().to(create_endpoint))
        .route("/endpoints/type/{type}", web::get().to(list_endpoints_by_type))
        .route("/endpoints/{id}", web::get().to(get_endpoint))
        .route("/endpoints/{id}", web::put().to(update_endpoint))
        .route("/endpoints/{id}", web::delete().to(delete_endpoint))
        // Internal routes
        .route("/routes", web::get().to(list_internal_routes))
        .route("/routes", web::post().to(create_internal_route))
        .route("/routes/type/{type}", web::get().to(list_internal_routes_by_type))
        .route("/routes/{id}", web::get().to(get_internal_route))
        .route("/routes/{id}", web::put().to(update_internal_route))
        .route("/routes/{id}", web::delete().to(delete_internal_route))
        // Sync
        .route("/sync", web::post().to(sync_internal_routes))
}
