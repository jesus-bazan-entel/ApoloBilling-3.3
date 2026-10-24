// src/main.rs
use actix_web::{web, App, HttpServer, middleware};
use std::sync::Arc;
use tracing::{info, error};

mod config;
mod error;
mod models;
mod database;
mod cache;
mod services;
mod esl;
mod api;

use config::Config;
use database::create_pool;
use cache::RedisClient;
use services::{
    AuthorizationService,
    ReservationManager,
    RealtimeBiller,
    CdrGenerator,
    CallSimulator,
    KamailioCdrSync,
};
use esl::{FreeSwitchCluster, EslServer};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .json()
        .init();

    info!("🚀 Starting Apolo Billing Engine (Rust)");

    // Load configuration
    let config = Config::from_env()
        .expect("Failed to load configuration");
    
    info!("Environment: {}", config.environment);

    // Create database pool
    let db_pool = create_pool(&config.database_url)
        .await
        .expect("Failed to create database pool");
    
    info!("✅ Database pool created");

    // Create Redis client
    let redis_client = RedisClient::new(&config.redis_url)
        .await
        .expect("Failed to create Redis client");
    
    info!("✅ Redis client connected");

    // Create services
    let reservation_mgr = Arc::new(ReservationManager::new(
        db_pool.clone(),
        redis_client.clone()
    ));

    let auth_service = Arc::new(AuthorizationService::new(
        db_pool.clone(),
        redis_client.clone(),
        reservation_mgr.clone()
    ));

    let realtime_biller = Arc::new(RealtimeBiller::new(
        redis_client.clone(),
        reservation_mgr.clone()
    ));

    let cdr_generator = Arc::new(CdrGenerator::new(
        db_pool.clone(),
        reservation_mgr.clone()
    ));

    // Create call simulator
    let call_simulator = Arc::new(CallSimulator::new(
        db_pool.clone(),
        redis_client.clone(),
        auth_service.clone(),
        cdr_generator.clone(),
        reservation_mgr.clone(),
    ));

    info!("✅ Call Simulator service created");

    // Start FreeSWITCH ESL cluster (client mode - connect to real FreeSWITCH)
    if !config.freeswitch_servers.is_empty() {
        let esl_cluster = FreeSwitchCluster::new(
            config.freeswitch_servers.clone(),
            auth_service.clone(),
            realtime_biller.clone(),
            cdr_generator.clone(),
            db_pool.clone(),
            redis_client.clone(),
        );

        tokio::spawn(async move {
            if let Err(e) = esl_cluster.start().await {
                error!("FreeSWITCH ESL cluster error: {}", e);
            }
        });

        info!("✅ FreeSWITCH ESL cluster started (client mode)");
    } else {
        info!("⚠️  No FreeSWITCH servers configured - starting ESL server for testing");
        
        // Start ESL Server (server mode - accept connections from simulator)
        let esl_server = EslServer::new(
            auth_service.clone(),
            realtime_biller.clone(),
            cdr_generator.clone(),
            db_pool.clone(),
            redis_client.clone(),
        );

        tokio::spawn(async move {
            if let Err(e) = esl_server.start("0.0.0.0:8021").await {
                error!("ESL Server error: {}", e);
            }
        });

        info!("✅ ESL Server started on 0.0.0.0:8021 (server mode for testing)");
    }

    // Kamailio dialog CDRs for inbound calls routed direct (no FreeSWITCH/ESL)
    if let Some(kamailio_url) = config.kamailio_database_url.clone() {
        match KamailioCdrSync::connect(db_pool.clone(), &kamailio_url).await {
            Ok(sync) => {
                tokio::spawn(sync.run());
            }
            Err(e) => error!("Kamailio CDR sync disabled - cannot connect to Kamailio DB: {}", e),
        }
    } else {
        info!("⚠️  KAMAILIO_DATABASE_URL not set - Kamailio CDR sync disabled");
    }

    // HTTP Server
    let bind_address = format!("{}:{}", config.host, config.port);
    info!("🌐 Starting HTTP server on {}", bind_address);

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(tracing_actix_web::TracingLogger::default())
            .app_data(web::Data::new(auth_service.clone()))
            .app_data(web::Data::new(reservation_mgr.clone()))
            .app_data(web::Data::new(cdr_generator.clone()))
            .app_data(web::Data::new(call_simulator.clone()))
            .configure(api::routes::configure)
    })
    .workers(8)  // 8 workers for maximum concurrency
    .bind(&bind_address)?
    .run()
    .await
}
