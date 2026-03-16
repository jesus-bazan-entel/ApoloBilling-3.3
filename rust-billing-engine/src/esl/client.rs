// src/esl/client.rs
use crate::config::FreeSwitchServer;
use crate::services::{AuthorizationService, RealtimeBiller, CdrGenerator};
use crate::esl::{EslConnection, EventHandler};
use crate::database::DbPool;  // ✅ Agregado
use crate::cache::RedisClient;  // ✅ Agregado
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, warn, error};

pub struct FreeSwitchCluster {
    servers: Vec<FreeSwitchServer>,
    auth_service: Arc<AuthorizationService>,
    realtime_biller: Arc<RealtimeBiller>,
    cdr_generator: Arc<CdrGenerator>,
    db_pool: DbPool,  // ✅ Agregado
    redis: RedisClient,  // ✅ Agregado
}

impl FreeSwitchCluster {
    pub fn new(
        servers: Vec<FreeSwitchServer>,
        auth_service: Arc<AuthorizationService>,
        realtime_biller: Arc<RealtimeBiller>,
        cdr_generator: Arc<CdrGenerator>,
        db_pool: DbPool,  // ✅ Agregado
        redis: RedisClient,  // ✅ Agregado
    ) -> Self {
        Self {
            servers,
            auth_service,
            realtime_biller,
            cdr_generator,
            db_pool,
            redis,
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for server in &self.servers {
            let server_clone = server.clone();
            let auth_service = self.auth_service.clone();
            let realtime_biller = self.realtime_biller.clone();
            let cdr_generator = self.cdr_generator.clone();
            let db_pool = self.db_pool.clone();  // ✅ Agregado
            let redis = self.redis.clone();  // ✅ Agregado

            tokio::spawn(async move {
                FreeSwitchClient::run(
                    server_clone,
                    auth_service,
                    realtime_biller,
                    cdr_generator,
                    db_pool,  // ✅ Agregado
                    redis,  // ✅ Agregado
                )
                .await;
            });
        }

        Ok(())
    }
}

struct FreeSwitchClient;

impl FreeSwitchClient {
    async fn run(
        server: FreeSwitchServer,
        auth_service: Arc<AuthorizationService>,
        realtime_biller: Arc<RealtimeBiller>,
        cdr_generator: Arc<CdrGenerator>,
        db_pool: DbPool,  // ✅ Agregado
        redis: RedisClient,  // ✅ Agregado
    ) {
        let server_id = format!("{}:{}", server.host, server.port);
        
        loop {
            info!("Connecting to FreeSWITCH ESL: {}", server_id);

            match Self::connect_and_listen(
                &server,
                &server_id,
                auth_service.clone(),
                realtime_biller.clone(),
                cdr_generator.clone(),
                db_pool.clone(),  // ✅ Agregado
                redis.clone(),  // ✅ Agregado
            )
            .await
            {
                Ok(_) => {
                    info!("FreeSWITCH ESL connection closed: {}", server_id);
                }
                Err(e) => {
                    error!("FreeSWITCH ESL error for {}: {}", server_id, e);
                }
            }

            warn!("Reconnecting to {} in 5 seconds...", server_id);
            sleep(Duration::from_secs(5)).await;
        }
    }

    async fn connect_and_listen(
        server: &FreeSwitchServer,
        server_id: &str,
        auth_service: Arc<AuthorizationService>,
        realtime_biller: Arc<RealtimeBiller>,
        cdr_generator: Arc<CdrGenerator>,
        db_pool: DbPool,  // ✅ Agregado
        redis: RedisClient,  // ✅ Agregado
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Connect to FreeSWITCH
        let connection = Arc::new(EslConnection::connect(
            &server.host,
            server.port,
            &server.password,
        ).await?);

        info!("✅ Connected and authenticated to FreeSWITCH: {}", server_id);

        // Event handler with shared connection
        let event_handler = EventHandler::new(
            server_id.to_string(),
            auth_service,
            realtime_biller,
            cdr_generator,
            db_pool,  // ✅ Agregado
            redis,  // ✅ Agregado
            connection.clone(),  // ✅ Pass connection for sending commands
        );

        // Event loop
        loop {
            match connection.read_event().await {
                Ok(Some(event)) => {
                    event_handler.handle_event(&event).await;
                }
                Ok(None) => {
                    // Not an event, might be a command response
                    continue;
                }
                Err(e) => {
                    error!("Error reading event from {}: {}", server_id, e);
                    break;
                }
            }
        }

        Ok(())
    }
}