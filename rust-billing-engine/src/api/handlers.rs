// src/api/handlers.rs
use actix_web::{web, HttpResponse};
use crate::services::{AuthorizationService, ReservationManager, CallSimulator, SimulateCallRequest, SimulationScenario};
use crate::models::{AuthRequest, ConsumeReservationRequest, HealthResponse};
use std::sync::Arc;

/// Query parameters for FreeSWITCH authorization (GET request)
#[derive(Debug, serde::Deserialize)]
pub struct FreeSwitchAuthQuery {
    pub caller: String,
    pub callee: String,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub direction: Option<String>,
}

pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(HealthResponse {
        status: "healthy".to_string(),
        service: "apolo-billing-engine".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

pub async fn authorize_call(
    req: web::Json<AuthRequest>,
    auth_service: web::Data<Arc<AuthorizationService>>,
) -> HttpResponse {
    match auth_service.authorize(&req).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => {
            tracing::error!("Authorization error: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "authorization_failed",
                "message": e.to_string()
            }))
        }
    }
}

/// FreeSWITCH authorization endpoint (GET with query params)
/// Returns plain text response for easy parsing in dialplan:
/// - "AUTHORIZED|max_seconds|rate_per_minute|account_id" on success
/// - "DENIED|reason" on failure
pub async fn freeswitch_authorize(
    query: web::Query<FreeSwitchAuthQuery>,
    auth_service: web::Data<Arc<AuthorizationService>>,
) -> HttpResponse {
    let auth_req = AuthRequest {
        caller: query.caller.clone(),
        callee: query.callee.clone(),
        uuid: query.uuid.clone(),
        direction: query.direction.clone(),
    };

    match auth_service.authorize(&auth_req).await {
        Ok(response) => {
            if response.authorized {
                // Format: AUTHORIZED|max_seconds|rate_per_minute|account_id|reservation_id
                let body = format!(
                    "AUTHORIZED|{}|{}|{}|{}",
                    response.max_duration_seconds.unwrap_or(3600),
                    response.rate_per_minute.unwrap_or(0.0),
                    response.account_id.unwrap_or(0),
                    response.reservation_id.map(|r| r.to_string()).unwrap_or_default()
                );
                HttpResponse::Ok()
                    .content_type("text/plain")
                    .body(body)
            } else {
                // Format: DENIED|reason
                let body = format!("DENIED|{}", response.reason);
                HttpResponse::Ok()
                    .content_type("text/plain")
                    .body(body)
            }
        }
        Err(e) => {
            tracing::error!("FreeSWITCH authorization error: {}", e);
            HttpResponse::Ok()
                .content_type("text/plain")
                .body(format!("DENIED|internal_error:{}", e))
        }
    }
}

pub async fn consume_reservation(
    req: web::Json<ConsumeReservationRequest>,
    reservation_mgr: web::Data<Arc<ReservationManager>>,
) -> HttpResponse {
    match reservation_mgr.consume_reservation(&req).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => {
            tracing::error!("Consume reservation error: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "consume_failed",
                "message": e.to_string()
            }))
        }
    }
}

// ==================== SIMULATION ENDPOINTS ====================

/// Start a simulated call
/// POST /api/v1/simulate/call
pub async fn simulate_call(
    req: web::Json<SimulateCallRequest>,
    simulator: web::Data<Arc<CallSimulator>>,
) -> HttpResponse {
    tracing::info!("📞 Simulation request: {} -> {}", req.caller, req.callee);

    match simulator.start_call(req.into_inner()).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => {
            tracing::error!("Simulation error: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "simulation_failed",
                "message": e
            }))
        }
    }
}

/// Hangup a simulated call
/// POST /api/v1/simulate/hangup/{call_uuid}
pub async fn simulate_hangup(
    path: web::Path<String>,
    query: web::Query<HangupQuery>,
    simulator: web::Data<Arc<CallSimulator>>,
) -> HttpResponse {
    let call_uuid = path.into_inner();
    tracing::info!("📞 Hangup request for call: {}", call_uuid);

    match simulator.hangup_call(&call_uuid, query.cause.clone()).await {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "message": format!("Call {} hung up", call_uuid)
        })),
        Err(e) => {
            tracing::error!("Hangup error: {}", e);
            HttpResponse::BadRequest().json(serde_json::json!({
                "error": "hangup_failed",
                "message": e
            }))
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct HangupQuery {
    #[serde(default)]
    pub cause: Option<String>,
}

/// List active simulated calls
/// GET /api/v1/simulate/calls
pub async fn list_simulated_calls(
    simulator: web::Data<Arc<CallSimulator>>,
) -> HttpResponse {
    let calls = simulator.list_active_calls().await;
    HttpResponse::Ok().json(serde_json::json!({
        "count": calls.len(),
        "calls": calls
    }))
}

/// Get a specific simulated call
/// GET /api/v1/simulate/call/{call_uuid}
pub async fn get_simulated_call(
    path: web::Path<String>,
    simulator: web::Data<Arc<CallSimulator>>,
) -> HttpResponse {
    let call_uuid = path.into_inner();

    match simulator.get_call(&call_uuid).await {
        Some(call) => HttpResponse::Ok().json(call),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "not_found",
            "message": format!("Call {} not found", call_uuid)
        }))
    }
}

/// Run a simulation scenario
/// POST /api/v1/simulate/scenario
pub async fn run_scenario(
    req: web::Json<SimulationScenario>,
    simulator: web::Data<Arc<CallSimulator>>,
) -> HttpResponse {
    tracing::info!("🎬 Running scenario: {}", req.name);

    let results = simulator.run_scenario(req.into_inner()).await;

    let successful = results.iter().filter(|r| r.success).count();
    let failed = results.len() - successful;

    HttpResponse::Ok().json(serde_json::json!({
        "scenario_completed": true,
        "total_calls": results.len(),
        "successful": successful,
        "failed": failed,
        "results": results
    }))
}

/// Cleanup completed simulations from memory
/// POST /api/v1/simulate/cleanup
pub async fn cleanup_simulations(
    simulator: web::Data<Arc<CallSimulator>>,
) -> HttpResponse {
    simulator.cleanup_completed().await;
    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "Completed simulations cleaned up"
    }))
}
