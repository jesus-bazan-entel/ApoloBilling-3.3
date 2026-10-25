// src/api/handlers.rs
use actix_web::HttpResponse;
use crate::models::HealthResponse;

pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(HealthResponse {
        status: "healthy".to_string(),
        service: "apolo-sbc-engine".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
