// src/services/realtime_biller.rs
use crate::cache::RedisClient;
use crate::services::ReservationManager;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{info, warn, error};

const EXTENSION_CHECK_INTERVAL: u64 = 180; // 3 minutes
const EXTENSION_THRESHOLD_SECONDS: i64 = 240; // Extend if less than 4 minutes remaining
const EXTENSION_MINUTES: i32 = 3; // Extend by 3 minutes each time

pub struct RealtimeBiller {
    redis: RedisClient,
    reservation_mgr: Arc<ReservationManager>,
    active_monitors: Arc<RwLock<HashMap<String, tokio::task::JoinHandle<()>>>>,
}

impl RealtimeBiller {
    pub fn new(
        redis: RedisClient,
        reservation_mgr: Arc<ReservationManager>,
    ) -> Self {
        Self {
            redis,
            reservation_mgr,
            active_monitors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start_billing(&self, call_uuid: String, account_type: String) {
        // Solo monitorear prepago
        if account_type != "prepaid" {
            info!("Skipping realtime billing for postpaid call: {}", call_uuid);
            return;
        }

        info!("✅ Starting realtime billing for call: {}", call_uuid);

        let redis = self.redis.clone();
        let call_uuid_clone = call_uuid.clone();
        let reservation_mgr_clone = self.reservation_mgr.clone();

        // Spawn monitoring task
        let handle = tokio::spawn(async move {
            Self::monitor_call(redis, call_uuid_clone, reservation_mgr_clone).await;
        });

        // Store handle
        let mut monitors = self.active_monitors.write().await;
        monitors.insert(call_uuid, handle);
    }

    pub async fn stop_billing(&self, call_uuid: &str) {
        let mut monitors = self.active_monitors.write().await;
        
        if let Some(handle) = monitors.remove(call_uuid) {
            handle.abort();
            info!("🛑 Stopped billing for call: {}", call_uuid);
        }
    }

    async fn monitor_call(redis: RedisClient, call_uuid: String, reservation_mgr: Arc<ReservationManager>) {
        let mut check_interval = interval(Duration::from_secs(EXTENSION_CHECK_INTERVAL));

        loop {
            check_interval.tick().await;

            // Check if call still active
            let session_key = format!("call_session:{}", call_uuid);
            match redis.get(&session_key).await {
                Ok(Some(session_data)) => {
                    // Parse session
                    if let Ok(session) = serde_json::from_str::<serde_json::Value>(&session_data) {
                        // Check duration vs max_duration
                        if let (Some(max_duration), Some(start_time_str)) = (
                            session["max_duration"].as_i64(),
                            session["start_time"].as_str(),
                        ) {
                            if let Ok(start_time) = chrono::DateTime::parse_from_rfc3339(start_time_str) {
                                let elapsed = (chrono::Utc::now() - start_time.with_timezone(&chrono::Utc)).num_seconds();
                                
                                let time_remaining = max_duration - elapsed;
                                
                                if time_remaining < EXTENSION_THRESHOLD_SECONDS {
                                    warn!(
                                        "⏱️ Call {} approaching max duration. Remaining: {}s",
                                        call_uuid, time_remaining
                                    );
                                    
                                    // ✅ Request extension
                                    match reservation_mgr.extend_reservation(&call_uuid, EXTENSION_MINUTES).await {
                                        Ok(result) => {
                                            if result.success {
                                                info!(
                                                    "✅ Reservation extended for call {}: +${}, new max duration: {}s",
                                                    call_uuid, result.additional_reserved, result.new_max_duration_seconds
                                                );
                                                
                                                // Update session with new max_duration
                                                let mut updated_session = session.clone();
                                                updated_session["max_duration"] = serde_json::json!(result.new_max_duration_seconds);
                                                
                                                if let Ok(updated_json) = serde_json::to_string(&updated_session) {
                                                    let _ = redis.set(&session_key, &updated_json, 3600).await;
                                                }
                                            } else {
                                                warn!(
                                                    "⚠️ Failed to extend reservation for call {}: {}",
                                                    call_uuid, result.reason
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            error!("❌ Error extending reservation for call {}: {}", call_uuid, e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(None) => {
                    info!("Call {} ended, stopping monitoring", call_uuid);
                    break;
                }
                Err(e) => {
                    error!("Error checking call {}: {}", call_uuid, e);
                    break;
                }
            }
        }
    }
}
