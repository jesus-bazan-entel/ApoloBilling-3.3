//! Example of how to integrate CDR API handlers into an Actix-web application
//!
//! This demonstrates the complete setup including routes, middleware, and configuration.

use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpServer};
use apolo_api::handlers::cdr::{export_cdrs, get_cdr, get_cdr_stats, list_cdrs};
use apolo_auth::jwt::JwtService;
use apolo_auth::middleware::AuthenticatedUser;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tracing::info;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Database connection
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://user:pass@localhost/apolo_billing".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    info!("Database pool created");

    // JWT service for authentication
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "your-secret-key".to_string());
    let jwt_service = Arc::new(JwtService::new(&jwt_secret, 3600));

    info!("Starting server on 0.0.0.0:8080");

    HttpServer::new(move || {
        App::new()
            // Add application data
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(jwt_service.clone()))
            // Configure CORS
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header()
                    .max_age(3600),
            )
            // Add logging middleware
            .wrap(Logger::default())
            // Configure routes
            .service(
                web::scope("/api/v1")
                    .service(configure_cdr_routes())
                    .service(configure_protected_routes()),
            )
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

/// Configure public CDR routes (requires authentication)
fn configure_cdr_routes() -> actix_web::Scope {
    web::scope("/cdrs")
        // List CDRs with filtering and pagination
        // GET /api/v1/cdrs?page=1&per_page=50&account_id=123
        .route("", web::get().to(list_cdrs))
        // Get single CDR by ID
        // GET /api/v1/cdrs/12345
        .route("/{id}", web::get().to(get_cdr))
        // Export CDRs in various formats
        // GET /api/v1/cdrs/export?format=csv&account_id=123&limit=100000
        .route("/export", web::get().to(export_cdrs))
        // Get CDR statistics
        // GET /api/v1/cdrs/stats?account_id=123&group_by=day
        .route("/stats", web::get().to(get_cdr_stats))
}

/// Configure protected routes (admin only)
fn configure_protected_routes() -> actix_web::Scope {
    web::scope("/admin")
        // Example: Protected route that requires authentication
        .route(
            "/cdrs",
            web::get().to(|_user: AuthenticatedUser| async {
                actix_web::HttpResponse::Ok().json(serde_json::json!({
                    "message": "Protected CDR access"
                }))
            }),
        )
}

/* Example API usage:

## List CDRs with pagination
curl -X GET "http://localhost:8080/api/v1/cdrs?page=1&per_page=50" \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"

## Filter CDRs by account and date range
curl -X GET "http://localhost:8080/api/v1/cdrs?account_id=123&start_date=2024-01-01&end_date=2024-01-31" \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"

## Get a specific CDR
curl -X GET "http://localhost:8080/api/v1/cdrs/12345" \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"

## Export CDRs to CSV
curl -X GET "http://localhost:8080/api/v1/cdrs/export?format=csv&account_id=123&limit=100000" \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -o cdrs_export.csv

## Export to JSON Lines format
curl -X GET "http://localhost:8080/api/v1/cdrs/export?format=jsonl&start_date=2024-01-01" \
  -H "Authorization: Bearer YOUR_JWT_TOKEN" \
  -o cdrs_export.jsonl

## Get statistics
curl -X GET "http://localhost:8080/api/v1/cdrs/stats?account_id=123&start_date=2024-01-01" \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"

## Get statistics grouped by day
curl -X GET "http://localhost:8080/api/v1/cdrs/stats?account_id=123&start_date=2024-01-01&end_date=2024-01-31&group_by=day" \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"

*/
