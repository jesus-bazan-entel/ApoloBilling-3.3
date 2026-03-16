// src/api/routes.rs
use actix_web::web;
use crate::api::handlers;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .route("/health", web::get().to(handlers::health_check))
            .route("/authorize", web::post().to(handlers::authorize_call))
            .route("/reservation/consume", web::post().to(handlers::consume_reservation))
            // FreeSWITCH-specific endpoint (GET with query params)
            .route("/freeswitch/authorize", web::get().to(handlers::freeswitch_authorize))
            // Simulation endpoints
            .route("/simulate/call", web::post().to(handlers::simulate_call))
            .route("/simulate/hangup/{call_uuid}", web::post().to(handlers::simulate_hangup))
            .route("/simulate/calls", web::get().to(handlers::list_simulated_calls))
            .route("/simulate/call/{call_uuid}", web::get().to(handlers::get_simulated_call))
            .route("/simulate/scenario", web::post().to(handlers::run_scenario))
            .route("/simulate/cleanup", web::post().to(handlers::cleanup_simulations))
    );
}
