// src/esl/event_handler.rs
use crate::services::{AuthorizationService, CdrGenerator};
use crate::models::AuthRequest;
use crate::esl::{event::EslEvent, connection::EslConnection};
use crate::database::DbPool;
use crate::cache::RedisClient;
use std::sync::Arc;
use chrono::Utc;
use tracing::{info, warn, error};

pub struct EventHandler {
    server_id: String,
    auth_service: Arc<AuthorizationService>,
    cdr_generator: Arc<CdrGenerator>,
    db_pool: DbPool,
    redis: RedisClient,
    connection: Option<Arc<EslConnection>>,  // Optional for server mode
}

impl EventHandler {
    pub fn new(
        server_id: String,
        auth_service: Arc<AuthorizationService>,
        cdr_generator: Arc<CdrGenerator>,
        db_pool: DbPool,
        redis: RedisClient,
        connection: Arc<EslConnection>,
    ) -> Self {
        Self {
            server_id,
            auth_service,
            cdr_generator,
            db_pool,
            redis,
            connection: Some(connection),
        }
    }

    pub fn new_server_mode(
        auth_service: Arc<AuthorizationService>,
        cdr_generator: Arc<CdrGenerator>,
        db_pool: DbPool,
        redis: RedisClient,
    ) -> Self {
        Self {
            server_id: "esl-server".to_string(),
            auth_service,
            cdr_generator,
            db_pool,
            redis,
            connection: None,  // No connection in server mode
        }
    }

    pub async fn handle_event(&self, event: &EslEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(event_name) = event.event_name() {
            match event_name.as_str() {
                "CHANNEL_CREATE" => self.handle_channel_create(event).await,
                "CHANNEL_ANSWER" => self.handle_channel_answer(event).await,
                "CHANNEL_HANGUP_COMPLETE" => self.handle_channel_hangup(event).await,
                _ => {}
            }
        }
        Ok(())
    }

    async fn handle_channel_create(&self, event: &EslEvent) {
        let uuid = match event.unique_id() {
            Some(id) => id.clone(),
            None => return,
        };

        // 🔍 Detectar si es a-leg o b-leg (solo procesar a-leg para evitar duplicados)
        let call_direction = event.get_header("Caller-Logical-Direction")
            .or_else(|| event.get_header("variable_leg"))
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        // Solo procesar el a-leg (origen de la llamada)
        if call_direction == "leg_b" || call_direction == "outbound" && event.get_header("Other-Leg-Unique-ID").is_some() {
            info!("⏭️  Skipping b-leg: {} (direction: {})", uuid, call_direction);
            return;
        }

        let caller = event.caller().cloned().unwrap_or_default();
        let callee = event.callee().cloned().unwrap_or_default();

        // 🔍 Detectar dirección de la llamada (inbound/outbound)
        // Si el contexto es from-pbx, la llamada es OUTBOUND (usuario de PBX llamando hacia afuera)
        let context = event.get_header("Caller-Context")
            .or_else(|| event.get_header("variable_context"))
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        let direction = if context == "from-pbx" || context.starts_with("from-pbx") {
            // Llamadas desde PBX siempre son salientes
            "outbound".to_string()
        } else {
            event.get_header("Call-Direction")
                .or_else(|| event.get_header("Caller-Direction"))
                .map(|s| s.to_lowercase())
                .unwrap_or_else(|| "outbound".to_string())
        };

        info!("📞 Context: {} → Direction: {}", context, direction);

        info!("📞 CHANNEL_CREATE (a-leg): {} - {} → {} [{}]", uuid, caller, callee, direction);

        // 🔍 Check if destination is a toll-free number (0800, 0801, 1800)
        // These numbers ARE billed to the account that owns them
        let is_toll_free = callee.starts_with("0800")
            || callee.starts_with("0801")
            || callee.starts_with("1800");

        if is_toll_free {
            info!("📞 Toll-free number detected: {} - will authorize for billing", callee);
        }

        // 🔍 Skip authorization for INBOUND calls (from external carriers via Kamailio)
        // EXCEPT for toll-free numbers which are billed to the receiving account
        // Inbound calls don't need billing authorization - the caller is external
        if (direction == "inbound" || context == "to-kamailio") && !is_toll_free {
            info!("⏭️  Skipping authorization for INBOUND call {} (context: {}, not toll-free)", uuid, context);

            // 🔒 Límite de llamadas simultáneas por endpoint (auth_mode=credentials)
            // también en entrantes: este path no pasa por authorize(), así que
            // el check va explícito aquí. Mismo kill path que los denies de billing.
            if let Some(reason) = self.auth_service.check_endpoint_concurrent_limit(&caller, &callee).await {
                warn!("❌ Inbound call DENIED (endpoint limit): {} - Reason: {}", uuid, reason);

                if let Some(conn) = &self.connection {
                    let kill_command = format!("api uuid_kill {} CALL_REJECTED\n\n", uuid);
                    match conn.send_command(&kill_command).await {
                        Ok(result) => {
                            info!("🔪 Sent kill command for call {}: {}", uuid, result.trim());
                        }
                        Err(e) => {
                            error!("❌ Failed to send kill command for call {}: {}", uuid, e);
                        }
                    }
                } else {
                    info!("📝 Call denied but no connection to send kill command (server mode)");
                }
                return;
            }

            // Insert into active_calls for tracking
            if let Ok(client) = self.db_pool.get().await {
                let now = Utc::now();
                let _ = client.execute(
                    "INSERT INTO active_calls (call_id, calling_number, called_number, direction, start_time, current_duration, current_cost, server)
                     VALUES ($1, $2, $3, $4, $5, 0, 0, $6)
                     ON CONFLICT (call_id) DO NOTHING",
                    &[&uuid, &caller, &callee, &"inbound".to_string(), &now, &self.server_id]
                ).await;
            }
            return;
        }

        // 🔍 Check if call was already authorized via HTTP endpoint (dialplan curl)
        // This prevents duplicate bucket reservations
        let already_authorized = match self.redis.get(&format!("bucket_hold:{}", uuid)).await {
            Ok(Some(_)) => {
                info!("⏭️  Call {} already authorized via HTTP, skipping ESL authorization", uuid);
                true
            }
            Ok(None) => false,
            Err(e) => {
                error!("Error checking existing bucket hold: {}", e);
                false
            }
        };

        if already_authorized {
            // Insert into active_calls if not already there
            if let Ok(client) = self.db_pool.get().await {
                let now = Utc::now();
                let _ = client.execute(
                    "INSERT INTO active_calls (call_id, calling_number, called_number, direction, start_time, current_duration, current_cost, server)
                     VALUES ($1, $2, $3, $4, $5, 0, 0, $6)
                     ON CONFLICT (call_id) DO NOTHING",
                    &[&uuid, &caller, &callee, &direction, &now, &self.server_id]
                ).await;
            }
            return;
        }

        // Authorize call (only if not already authorized via HTTP)
        let source_ip = event.source_ip();
        let auth_req = AuthRequest {
            caller: caller.clone(),
            callee: callee.clone(),
            uuid: Some(uuid.clone()),
            direction: Some(direction.clone()),
            source_ip: source_ip.clone(),
        };

        match self.auth_service.authorize(&auth_req).await {
            Ok(response) => {
                if !response.authorized {
                    warn!("❌ Call DENIED: {} - Reason: {}", uuid, response.reason);
                    
                    // Send uuid_kill command to FreeSWITCH (if connection available)
                    if let Some(conn) = &self.connection {
                        let kill_command = format!("api uuid_kill {} CALL_REJECTED\n\n", uuid);
                        match conn.send_command(&kill_command).await {
                            Ok(result) => {
                                info!("🔪 Sent kill command for call {}: {}", uuid, result.trim());
                            }
                            Err(e) => {
                                error!("❌ Failed to send kill command for call {}: {}", uuid, e);
                            }
                        }
                    } else {
                        info!("📝 Call denied but no connection to send kill command (server mode)");
                    }

                } else {
                    info!("✅ Call AUTHORIZED: {} [{}]", uuid, direction);

                    // Insert into active_calls table
                    if let Ok(client) = self.db_pool.get().await {
                        let now = Utc::now();

                        match client.execute(
                            "INSERT INTO active_calls (call_id, calling_number, called_number, direction, start_time, current_duration, current_cost, server)
                             VALUES ($1, $2, $3, $4, $5, 0, 0, $6)
                             ON CONFLICT (call_id) DO NOTHING",
                            &[&uuid, &caller, &callee, &direction, &now, &self.server_id]
                        ).await {
                            Ok(_) => info!("📊 Added call {} to active_calls [{}]", uuid, direction),
                            Err(e) => error!("❌ Failed to add call {} to active_calls: {}", uuid, e),
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error authorizing call {}: {}", uuid, e);
            }
        }
    }

    async fn handle_channel_answer(&self, event: &EslEvent) {
        let uuid = match event.unique_id() {
            Some(id) => id.clone(),
            None => return,
        };

        // 🔍 Solo procesar a-leg para evitar duplicados
        let call_direction = event.get_header("Caller-Logical-Direction")
            .or_else(|| event.get_header("variable_leg"))
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        if call_direction == "leg_b" || call_direction == "outbound" && event.get_header("Other-Leg-Unique-ID").is_some() {
            info!("⏭️  Skipping b-leg answer: {} (direction: {})", uuid, call_direction);
            return;
        }

        info!("✅ CHANNEL_ANSWER (a-leg): {}", uuid);

        // Update active_calls table with answer_time and status
        if let Ok(client) = self.db_pool.get().await {
            let now = Utc::now();
            match client.execute(
                "UPDATE active_calls SET answer_time = $1, status = 'answered' WHERE call_id = $2",
                &[&now, &uuid]
            ).await {
                Ok(rows) => {
                    if rows > 0 {
                        info!("📞 Updated call {} to answered status", uuid);
                    }
                }
                Err(e) => error!("❌ Failed to update answer_time for call {}: {}", uuid, e),
            }
        }

    }

    async fn handle_channel_hangup(&self, event: &EslEvent) {
        let uuid = match event.unique_id() {
            Some(id) => id.clone(),
            None => return,
        };

        // 🔍 Solo procesar a-leg para evitar duplicados
        let call_direction = event.get_header("Caller-Logical-Direction")
            .or_else(|| event.get_header("variable_leg"))
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        if call_direction == "leg_b" || call_direction == "outbound" && event.get_header("Other-Leg-Unique-ID").is_some() {
            info!("⏭️  Skipping b-leg hangup: {} (direction: {})", uuid, call_direction);
            return;
        }

        let caller = event.caller().cloned().unwrap_or_default();
        let callee = event.callee().cloned().unwrap_or_default();
        let duration = event.duration().unwrap_or(0);
        let billsec = event.billsec().unwrap_or(0);
        let hangup_cause = event.hangup_cause().cloned().unwrap_or_else(|| "UNKNOWN".to_string());

        // 🔍 Detectar dirección real
        // Si el contexto es from-pbx, la llamada es OUTBOUND
        let context = event.get_header("Caller-Context")
            .or_else(|| event.get_header("variable_context"))
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        let direction = if context == "from-pbx" || context.starts_with("from-pbx") {
            "outbound".to_string()
        } else {
            event.get_header("Call-Direction")
                .or_else(|| event.get_header("Caller-Direction"))
                .map(|s| s.to_lowercase())
                .unwrap_or_else(|| "outbound".to_string())
        };

        info!(
            "📴 CHANNEL_HANGUP (a-leg): {} - Duration: {}s, Billsec: {}s, Cause: {} [{}]",
            uuid, duration, billsec, hangup_cause, direction
        );

        // Remove from active_calls table
        if let Ok(client) = self.db_pool.get().await {
            match client.execute(
                "DELETE FROM active_calls WHERE call_id = $1",
                &[&uuid]
            ).await {
                Ok(rows) => {
                    if rows > 0 {
                        info!("📊 Removed call {} from active_calls", uuid);
                    }
                }
                Err(e) => error!("❌ Failed to remove call {} from active_calls: {}", uuid, e),
            }
        }

        // ✅ Limpia Redis para llamadas rechazadas (que nunca tuvieron bucket hold)
        match self.redis.get(&format!("bucket_hold:{}", uuid)).await {
            Ok(None) => {
                // No hay bucket hold, limpia cualquier estado residual en Redis
                info!("🧹 Cleaning up Redis for non-reserved call: {}", uuid);
                let _ = self.redis.delete(&format!("call_state:{}", uuid)).await;
            }
            Ok(Some(_)) => {
                // Hay bucket hold, el CDR (consume()) se encargará de limpiarlo
            }
            Err(e) => {
                error!("Error checking bucket hold for {}: {}", uuid, e);
            }
        }

        // Generate CDR
        let source_ip = event.source_ip();
        let hangup_event = crate::services::cdr_generator::HangupEvent {
            uuid: uuid.clone(),
            caller,
            callee,
            start_time: event.start_time().unwrap_or_else(Utc::now),  // ✅ Parse from event
            answer_time: event.answer_time(),  // ✅ Parse from event
            end_time: event.end_time().unwrap_or_else(Utc::now),  // ✅ Parse from event
            duration,
            billsec,
            hangup_cause,
            direction: direction.clone(),  // ✅ Dirección real
            server_id: self.server_id.clone(),
            source_ip,
        };

        if let Err(e) = self.cdr_generator.generate_cdr(hangup_event).await {
            error!("Error generating CDR for {}: {}", uuid, e);
        }
    }
}