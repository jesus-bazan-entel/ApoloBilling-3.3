// src/api/routes.rs
use actix_web::web;
use crate::api::handlers;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/api/v1").route("/health", web::get().to(handlers::health_check)));
}
