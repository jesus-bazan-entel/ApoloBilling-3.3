//! ApoloBilling Rust Backend Server
//!
//! High-performance backend for handling CDR queries, exports, and statistics.
//! Designed to handle millions of CDRs with efficient streaming exports.

use actix_cors::Cors;
use actix_web::{http::header, middleware, web, App, HttpResponse, HttpServer};
use apolo_api::handlers::{
    cdr, configure_accounts, configure_active_calls, configure_audit, configure_auth,
    configure_dashboard, configure_dialplan, configure_management, configure_plans,
    configure_rate_cards, configure_rates, configure_reservations, configure_settings,
    configure_stats, configure_users, create_cdr, ws_handler,
};
use apolo_auth::{JwtService, PasswordService};
use apolo_db::create_pool;
use std::env;
use std::sync::Arc;
use tracing::{info, warn, Level};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Health check endpoint
async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "apolo-billing-rust",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// Configure API routes
fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            // Health check
            .route("/health", web::get().to(health_check))
            // Dashboard stats
            .configure(configure_dashboard)
            // Statistics endpoints
            .configure(configure_stats)
            // Auth endpoints
            .configure(configure_auth)
            // User management endpoints (superadmin only)
            .configure(configure_users)
            // Audit log endpoints (superadmin only)
            .configure(configure_audit)
            // Account endpoints
            .configure(configure_accounts)
            // Rate card endpoints
            .configure(configure_rate_cards)
            // Legacy rates endpoints
            .configure(configure_rates)
            // Plan endpoints
            .configure(configure_plans)
            // Active calls endpoints
            .configure(configure_active_calls)
            // Reservations endpoints
            .configure(configure_reservations)
            // Zone/Prefix/Tariff management endpoints
            .configure(configure_management)
            // Dialplan FreeSWITCH management (superadmin only)
            .configure(configure_dialplan)
            // System settings (read: authenticated; write: superadmin only)
            .configure(configure_settings)
            // CDR endpoints - high-volume operations
            .service(
                web::scope("/cdrs")
                    .route("", web::get().to(cdr::list_cdrs))
                    .route("", web::post().to(create_cdr))
                    .route("/export", web::get().to(cdr::export_cdrs))
                    .route("/stats", web::get().to(cdr::get_cdr_stats))
                    .route("/{id}", web::get().to(cdr::get_cdr)),
            ),
    );
}

/// Initialize tracing/logging
fn init_tracing() {
    let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(format!(
            "apolo_billing={},apolo_api={},apolo_db={},actix_web=info,sqlx=warn",
            log_level, log_level, log_level
        ))
    });

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            fmt::layer()
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true),
        )
        .init();
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Initialize logging
    init_tracing();

    info!(
        "Starting ApoloBilling Rust Backend v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Load configuration from environment
    let host = env::var("RUST_SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = env::var("RUST_SERVER_PORT")
        .unwrap_or_else(|_| "9001".to_string())
        .parse()
        .expect("RUST_SERVER_PORT must be a valid port number");
    let workers: usize = env::var("RUST_SERVER_WORKERS")
        .unwrap_or_else(|_| num_cpus::get().to_string())
        .parse()
        .unwrap_or_else(|_| num_cpus::get());

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set (e.g., postgresql://user:pass@localhost/apolo_billing)");

    let max_connections: u32 = env::var("DATABASE_MAX_CONNECTIONS")
        .unwrap_or_else(|_| "20".to_string())
        .parse()
        .unwrap_or(20);

    // JWT configuration
    let jwt_secret = env::var("JWT_SECRET")
        .unwrap_or_else(|_| "apolo-billing-secret-key-change-in-production".to_string());
    let jwt_expiration: i64 = env::var("JWT_EXPIRATION_SECS")
        .unwrap_or_else(|_| "1800".to_string()) // 30 minutes default
        .parse()
        .unwrap_or(1800);

    // Create auth services
    let jwt_service = Arc::new(JwtService::new(&jwt_secret, jwt_expiration));
    let password_service = Arc::new(PasswordService::new());

    info!(
        "JWT service configured with {} second token expiration",
        jwt_expiration
    );

    // CORS configuration
    let cors_origins = env::var("CORS_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:3000,http://127.0.0.1:3000".to_string());

    info!("Connecting to database...");
    let pool = create_pool(&database_url, Some(max_connections))
        .await
        .expect("Failed to create database pool");

    info!(
        "Database connection established with {} max connections",
        max_connections
    );

    let bind_addr = format!("{}:{}", host, port);
    info!(
        "Starting HTTP server on {} with {} workers",
        bind_addr, workers
    );

    // Clone services for closure
    let jwt_service_clone = jwt_service.clone();
    let password_service_clone = password_service.clone();

    // Create and run server
    HttpServer::new(move || {
        // Configure CORS - clone cors_origins for each worker
        let cors_origins_inner = cors_origins.clone();
        let cors = Cors::default()
            .allowed_origin_fn(move |origin, _req_head| {
                let origins: Vec<&str> = cors_origins_inner.split(',').collect();
                if let Ok(origin_str) = origin.to_str() {
                    origins.iter().any(|o| o.trim() == origin_str)
                } else {
                    false
                }
            })
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
            .allowed_headers(vec![
                header::AUTHORIZATION,
                header::ACCEPT,
                header::CONTENT_TYPE,
                header::COOKIE,
            ])
            .supports_credentials()
            .max_age(3600);

        App::new()
            // Add database pool to app data
            .app_data(web::Data::new(pool.clone()))
            // Add auth services
            .app_data(web::Data::new(jwt_service_clone.clone()))
            .app_data(web::Data::new(password_service_clone.clone()))
            // Configure payload limits for large exports
            .app_data(web::PayloadConfig::new(10 * 1024 * 1024)) // 10MB max payload
            .app_data(web::QueryConfig::default().error_handler(|err, _req| {
                let error_message = err.to_string();
                actix_web::error::InternalError::from_response(
                    err,
                    HttpResponse::BadRequest().json(serde_json::json!({
                        "error": "invalid_query",
                        "message": error_message
                    })),
                )
                .into()
            }))
            // Middleware
            .wrap(cors)
            .wrap(middleware::Logger::new("%a \"%r\" %s %b %Dms"))
            .wrap(middleware::Compress::default())
            .wrap(middleware::NormalizePath::trim())
            // Configure routes
            .configure(configure_routes)
            // WebSocket endpoint for real-time updates
            .route("/ws", web::get().to(ws_handler))
            // Root redirect to health
            .route(
                "/",
                web::get().to(|| async {
                    HttpResponse::Found()
                        .append_header(("Location", "/api/v1/health"))
                        .finish()
                }),
            )
    })
    .workers(workers)
    .bind(&bind_addr)?
    .run()
    .await
}
