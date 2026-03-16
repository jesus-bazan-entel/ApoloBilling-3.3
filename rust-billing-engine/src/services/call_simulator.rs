// src/services/call_simulator.rs
//! Call Simulator for testing billing flows without FreeSWITCH
//!
//! Simulates the complete call lifecycle:
//! 1. CHANNEL_CREATE - Authorization and reservation
//! 2. CHANNEL_ANSWER - Call connected
//! 3. CHANNEL_HANGUP_COMPLETE - CDR generation and balance consumption

use crate::services::{AuthorizationService, CdrGenerator, ReservationManager};
use crate::cache::RedisClient;
use crate::models::{AuthRequest, AuthResponse};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use uuid::Uuid;
use std::collections::HashMap;

/// Simulated call state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatedCall {
    pub call_uuid: String,
    pub caller: String,
    pub callee: String,
    pub direction: String,
    pub start_time: DateTime<Utc>,
    pub answer_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub status: CallStatus,
    pub account_id: Option<i32>,
    pub reservation_id: Option<String>,
    pub rate_per_minute: Option<Decimal>,
    pub max_duration_seconds: Option<i32>,
    pub hangup_cause: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CallStatus {
    Ringing,
    Answered,
    Completed,
    Failed,
}

impl std::fmt::Display for CallStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallStatus::Ringing => write!(f, "ringing"),
            CallStatus::Answered => write!(f, "answered"),
            CallStatus::Completed => write!(f, "completed"),
            CallStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Request to start a simulated call
#[derive(Debug, Clone, Deserialize)]
pub struct SimulateCallRequest {
    /// Calling number (ANI) - used to identify the account
    pub caller: String,
    /// Called number (DNIS) - used for rate lookup
    pub callee: String,
    /// Call direction: "inbound" or "outbound"
    #[serde(default = "default_direction")]
    pub direction: String,
    /// Simulated call duration in seconds (if not provided, call stays active)
    pub duration_seconds: Option<i32>,
    /// Auto-answer after this many seconds (default: 2)
    #[serde(default = "default_ring_time")]
    pub ring_seconds: i32,
    /// Hangup cause (default: NORMAL_CLEARING)
    #[serde(default = "default_hangup_cause")]
    pub hangup_cause: String,
}

fn default_direction() -> String {
    "outbound".to_string()
}

fn default_ring_time() -> i32 {
    2
}

fn default_hangup_cause() -> String {
    "NORMAL_CLEARING".to_string()
}

/// Response from starting a simulated call
#[derive(Debug, Serialize)]
pub struct SimulateCallResponse {
    pub success: bool,
    pub call_uuid: String,
    pub message: String,
    pub authorization: Option<AuthorizationResult>,
}

#[derive(Debug, Serialize)]
pub struct AuthorizationResult {
    pub authorized: bool,
    pub reason: String,
    pub account_id: Option<i32>,
    pub reservation_id: Option<String>,
    pub reserved_amount: Option<Decimal>,
    pub rate_per_minute: Option<Decimal>,
    pub max_duration_seconds: Option<i32>,
}

/// Simulation scenario for batch testing
#[derive(Debug, Clone, Deserialize)]
pub struct SimulationScenario {
    pub name: String,
    pub calls: Vec<ScenarioCall>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScenarioCall {
    pub caller: String,
    pub callee: String,
    #[serde(default = "default_direction")]
    pub direction: String,
    pub duration_seconds: i32,
    #[serde(default)]
    pub delay_before_ms: u64,
}

/// Call simulator service
pub struct CallSimulator {
    db_pool: deadpool_postgres::Pool,
    redis_client: RedisClient,
    auth_service: Arc<AuthorizationService>,
    cdr_generator: Arc<CdrGenerator>,
    reservation_manager: Arc<ReservationManager>,
    active_simulations: Arc<RwLock<HashMap<String, SimulatedCall>>>,
    server_id: String,
}

impl CallSimulator {
    pub fn new(
        db_pool: deadpool_postgres::Pool,
        redis_client: RedisClient,
        auth_service: Arc<AuthorizationService>,
        cdr_generator: Arc<CdrGenerator>,
        reservation_manager: Arc<ReservationManager>,
    ) -> Self {
        Self {
            db_pool,
            redis_client,
            auth_service,
            cdr_generator,
            reservation_manager,
            active_simulations: Arc::new(RwLock::new(HashMap::new())),
            server_id: "simulator-001".to_string(),
        }
    }

    /// Start a simulated call
    pub async fn start_call(&self, req: SimulateCallRequest) -> Result<SimulateCallResponse, String> {
        let call_uuid = Uuid::new_v4().to_string();
        let start_time = Utc::now();

        info!(
            "📞 SIMULATOR: Starting {} call {} -> {} (uuid: {})",
            req.direction, req.caller, req.callee, call_uuid
        );

        // Phase 1: CHANNEL_CREATE - Authorization
        let auth_request = AuthRequest {
            caller: req.caller.clone(),
            callee: req.callee.clone(),
            uuid: Some(call_uuid.clone()),
            direction: Some(req.direction.clone()),
        };

        let auth_result = self.auth_service.authorize(&auth_request).await;

        match auth_result {
            Ok(auth) => {
                if !auth.authorized {
                    warn!(
                        "❌ SIMULATOR: Call {} DENIED - {}",
                        call_uuid, auth.reason
                    );
                    return Ok(SimulateCallResponse {
                        success: false,
                        call_uuid,
                        message: format!("Call denied: {}", auth.reason),
                        authorization: Some(AuthorizationResult {
                            authorized: false,
                            reason: auth.reason,
                            account_id: auth.account_id.map(|id| id as i32),
                            reservation_id: auth.reservation_id.map(|id| id.to_string()),
                            reserved_amount: auth.reserved_amount.map(Decimal::from_f64_retain).flatten(),
                            rate_per_minute: auth.rate_per_minute.map(Decimal::from_f64_retain).flatten(),
                            max_duration_seconds: auth.max_duration_seconds,
                        }),
                    });
                }

                info!(
                    "✅ SIMULATOR: Call {} AUTHORIZED - account_id: {:?}, rate: {:?}/min",
                    call_uuid, auth.account_id, auth.rate_per_minute
                );

                // Insert into active_calls table
                if let Err(e) = self.insert_active_call(&call_uuid, &req, &auth, start_time).await {
                    error!("Failed to insert active call: {}", e);
                }

                // Create simulated call record
                let sim_call = SimulatedCall {
                    call_uuid: call_uuid.clone(),
                    caller: req.caller.clone(),
                    callee: req.callee.clone(),
                    direction: req.direction.clone(),
                    start_time,
                    answer_time: None,
                    end_time: None,
                    status: CallStatus::Ringing,
                    account_id: auth.account_id.map(|id| id as i32),
                    reservation_id: auth.reservation_id.map(|id| id.to_string()),
                    rate_per_minute: auth.rate_per_minute.map(Decimal::from_f64_retain).flatten(),
                    max_duration_seconds: auth.max_duration_seconds,
                    hangup_cause: None,
                };

                // Store in active simulations
                {
                    let mut sims = self.active_simulations.write().await;
                    sims.insert(call_uuid.clone(), sim_call.clone());
                }

                // Spawn async task to handle call lifecycle
                let simulator = self.clone_refs();
                let req_clone = req.clone();
                let uuid_clone = call_uuid.clone();

                tokio::spawn(async move {
                    simulator.run_call_lifecycle(uuid_clone, req_clone).await;
                });

                Ok(SimulateCallResponse {
                    success: true,
                    call_uuid,
                    message: "Call started successfully".to_string(),
                    authorization: Some(AuthorizationResult {
                        authorized: true,
                        reason: auth.reason,
                        account_id: auth.account_id.map(|id| id as i32),
                        reservation_id: auth.reservation_id.map(|id| id.to_string()),
                        reserved_amount: auth.reserved_amount.map(Decimal::from_f64_retain).flatten(),
                        rate_per_minute: auth.rate_per_minute.map(Decimal::from_f64_retain).flatten(),
                        max_duration_seconds: auth.max_duration_seconds,
                    }),
                })
            }
            Err(e) => {
                error!("❌ SIMULATOR: Authorization error for {}: {}", call_uuid, e);
                Err(format!("Authorization error: {}", e))
            }
        }
    }

    /// Run the complete call lifecycle asynchronously
    async fn run_call_lifecycle(&self, call_uuid: String, req: SimulateCallRequest) {
        // Wait for ring time
        tokio::time::sleep(tokio::time::Duration::from_secs(req.ring_seconds as u64)).await;

        // Phase 2: CHANNEL_ANSWER
        let answer_time = Utc::now();
        {
            let mut sims = self.active_simulations.write().await;
            if let Some(call) = sims.get_mut(&call_uuid) {
                call.answer_time = Some(answer_time);
                call.status = CallStatus::Answered;
            }
        }

        info!("📞 SIMULATOR: Call {} ANSWERED", call_uuid);

        // Wait for call duration (if specified)
        if let Some(duration) = req.duration_seconds {
            tokio::time::sleep(tokio::time::Duration::from_secs(duration as u64)).await;
        } else {
            // If no duration specified, wait for manual hangup or max 10 minutes
            tokio::time::sleep(tokio::time::Duration::from_secs(600)).await;
        }

        // Phase 3: CHANNEL_HANGUP_COMPLETE
        let end_time = Utc::now();
        let hangup_cause = req.hangup_cause.clone();

        // Get call data
        let call_data = {
            let sims = self.active_simulations.read().await;
            sims.get(&call_uuid).cloned()
        };

        if let Some(call) = call_data {
            // Calculate duration and billsec
            let duration = if let Some(answer) = call.answer_time {
                (end_time - answer).num_seconds() as i32
            } else {
                0
            };
            let billsec = duration;

            info!(
                "📞 SIMULATOR: Call {} HANGUP - duration: {}s, cause: {}",
                call_uuid, duration, hangup_cause
            );

            // Remove from active_calls table
            if let Err(e) = self.remove_active_call(&call_uuid).await {
                error!("Failed to remove active call: {}", e);
            }

            // Generate CDR
            let hangup_event = crate::services::cdr_generator::HangupEvent {
                uuid: call_uuid.clone(),
                caller: call.caller.clone(),
                callee: call.callee.clone(),
                direction: call.direction.clone(),
                start_time: call.start_time,
                answer_time: call.answer_time,
                end_time,
                duration,
                billsec,
                hangup_cause: hangup_cause.clone(),
                server_id: self.server_id.clone(),
            };

            match self.cdr_generator.generate_cdr(hangup_event).await {
                Ok(cdr_id) => {
                    info!("✅ SIMULATOR: CDR {} generated for call {}", cdr_id, call_uuid);
                }
                Err(e) => {
                    error!("❌ SIMULATOR: Failed to generate CDR for {}: {}", call_uuid, e);
                }
            }

            // Update simulation status
            {
                let mut sims = self.active_simulations.write().await;
                if let Some(call) = sims.get_mut(&call_uuid) {
                    call.end_time = Some(end_time);
                    call.status = CallStatus::Completed;
                    call.hangup_cause = Some(hangup_cause);
                }
            }
        }
    }

    /// Insert call into active_calls table
    async fn insert_active_call(
        &self,
        call_uuid: &str,
        req: &SimulateCallRequest,
        auth: &AuthResponse,
        start_time: DateTime<Utc>,
    ) -> Result<(), String> {
        let client = self.db_pool.get().await.map_err(|e| e.to_string())?;
        let account_id: Option<i32> = auth.account_id.map(|id| id as i32);

        client.execute(
            r#"
            INSERT INTO active_calls (
                call_id, calling_number, called_number, direction,
                start_time, current_duration, current_cost, server,
                client_id, last_updated
            )
            VALUES ($1, $2, $3, $4, $5, 0, 0, $6, $7, $8)
            ON CONFLICT (call_id) DO NOTHING
            "#,
            &[
                &call_uuid,
                &req.caller,
                &req.callee,
                &req.direction,
                &start_time,
                &self.server_id,
                &account_id,
                &start_time,
            ],
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Remove call from active_calls table
    async fn remove_active_call(&self, call_uuid: &str) -> Result<(), String> {
        let client = self.db_pool.get().await.map_err(|e| e.to_string())?;

        client.execute(
            "DELETE FROM active_calls WHERE call_id = $1",
            &[&call_uuid],
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Hangup a specific call
    pub async fn hangup_call(&self, call_uuid: &str, hangup_cause: Option<String>) -> Result<(), String> {
        let cause = hangup_cause.unwrap_or_else(|| "NORMAL_CLEARING".to_string());

        let call_data = {
            let sims = self.active_simulations.read().await;
            sims.get(call_uuid).cloned()
        };

        if let Some(call) = call_data {
            if call.status == CallStatus::Completed {
                return Err("Call already completed".to_string());
            }

            let end_time = Utc::now();
            let duration = if let Some(answer) = call.answer_time {
                (end_time - answer).num_seconds() as i32
            } else {
                0
            };

            info!(
                "📞 SIMULATOR: Manual hangup for call {} - duration: {}s",
                call_uuid, duration
            );

            // Remove from active_calls
            self.remove_active_call(call_uuid).await?;

            // Generate CDR
            let hangup_event = crate::services::cdr_generator::HangupEvent {
                uuid: call_uuid.to_string(),
                caller: call.caller.clone(),
                callee: call.callee.clone(),
                direction: call.direction.clone(),
                start_time: call.start_time,
                answer_time: call.answer_time,
                end_time,
                duration,
                billsec: duration,
                hangup_cause: cause.clone(),
                server_id: self.server_id.clone(),
            };

            self.cdr_generator.generate_cdr(hangup_event).await
                .map_err(|e| e.to_string())?;

            // Update status
            {
                let mut sims = self.active_simulations.write().await;
                if let Some(call) = sims.get_mut(call_uuid) {
                    call.end_time = Some(end_time);
                    call.status = CallStatus::Completed;
                    call.hangup_cause = Some(cause);
                }
            }

            Ok(())
        } else {
            Err(format!("Call {} not found", call_uuid))
        }
    }

    /// Get list of active simulated calls
    pub async fn list_active_calls(&self) -> Vec<SimulatedCall> {
        let sims = self.active_simulations.read().await;
        sims.values()
            .filter(|c| c.status != CallStatus::Completed && c.status != CallStatus::Failed)
            .cloned()
            .collect()
    }

    /// Get call by UUID
    pub async fn get_call(&self, call_uuid: &str) -> Option<SimulatedCall> {
        let sims = self.active_simulations.read().await;
        sims.get(call_uuid).cloned()
    }

    /// Run a complete simulation scenario
    pub async fn run_scenario(&self, scenario: SimulationScenario) -> Vec<SimulateCallResponse> {
        info!("🎬 SIMULATOR: Running scenario '{}' with {} calls", scenario.name, scenario.calls.len());

        let mut results = Vec::new();

        for (i, call) in scenario.calls.iter().enumerate() {
            if call.delay_before_ms > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(call.delay_before_ms)).await;
            }

            info!("🎬 SIMULATOR: Scenario call {}/{}", i + 1, scenario.calls.len());

            let req = SimulateCallRequest {
                caller: call.caller.clone(),
                callee: call.callee.clone(),
                direction: call.direction.clone(),
                duration_seconds: Some(call.duration_seconds),
                ring_seconds: 1,
                hangup_cause: "NORMAL_CLEARING".to_string(),
            };

            match self.start_call(req).await {
                Ok(response) => results.push(response),
                Err(e) => {
                    results.push(SimulateCallResponse {
                        success: false,
                        call_uuid: "".to_string(),
                        message: e,
                        authorization: None,
                    });
                }
            }
        }

        info!("🎬 SIMULATOR: Scenario '{}' completed", scenario.name);
        results
    }

    /// Clear completed calls from memory
    pub async fn cleanup_completed(&self) {
        let mut sims = self.active_simulations.write().await;
        sims.retain(|_, call| {
            call.status != CallStatus::Completed && call.status != CallStatus::Failed
        });
    }

    /// Clone references for spawning tasks
    fn clone_refs(&self) -> CallSimulatorRefs {
        CallSimulatorRefs {
            db_pool: self.db_pool.clone(),
            redis_client: self.redis_client.clone(),
            auth_service: self.auth_service.clone(),
            cdr_generator: self.cdr_generator.clone(),
            reservation_manager: self.reservation_manager.clone(),
            active_simulations: self.active_simulations.clone(),
            server_id: self.server_id.clone(),
        }
    }
}

/// Lightweight reference holder for async tasks
struct CallSimulatorRefs {
    db_pool: deadpool_postgres::Pool,
    redis_client: RedisClient,
    auth_service: Arc<AuthorizationService>,
    cdr_generator: Arc<CdrGenerator>,
    reservation_manager: Arc<ReservationManager>,
    active_simulations: Arc<RwLock<HashMap<String, SimulatedCall>>>,
    server_id: String,
}

impl CallSimulatorRefs {
    async fn run_call_lifecycle(&self, call_uuid: String, req: SimulateCallRequest) {
        // Wait for ring time
        tokio::time::sleep(tokio::time::Duration::from_secs(req.ring_seconds as u64)).await;

        // Phase 2: CHANNEL_ANSWER
        let answer_time = Utc::now();
        {
            let mut sims = self.active_simulations.write().await;
            if let Some(call) = sims.get_mut(&call_uuid) {
                call.answer_time = Some(answer_time);
                call.status = CallStatus::Answered;
            }
        }

        info!("📞 SIMULATOR: Call {} ANSWERED", call_uuid);

        // Wait for call duration (if specified)
        if let Some(duration) = req.duration_seconds {
            tokio::time::sleep(tokio::time::Duration::from_secs(duration as u64)).await;
        } else {
            // If no duration specified, wait for manual hangup or max 10 minutes
            tokio::time::sleep(tokio::time::Duration::from_secs(600)).await;
        }

        // Phase 3: CHANNEL_HANGUP_COMPLETE
        let end_time = Utc::now();
        let hangup_cause = req.hangup_cause.clone();

        // Get call data
        let call_data = {
            let sims = self.active_simulations.read().await;
            sims.get(&call_uuid).cloned()
        };

        if let Some(call) = call_data {
            // Calculate duration and billsec
            let duration = if let Some(answer) = call.answer_time {
                (end_time - answer).num_seconds() as i32
            } else {
                0
            };
            let billsec = duration;

            info!(
                "📞 SIMULATOR: Call {} HANGUP - duration: {}s, cause: {}",
                call_uuid, duration, hangup_cause
            );

            // Remove from active_calls table
            if let Err(e) = self.remove_active_call(&call_uuid).await {
                error!("Failed to remove active call: {}", e);
            }

            // Generate CDR
            let hangup_event = crate::services::cdr_generator::HangupEvent {
                uuid: call_uuid.clone(),
                caller: call.caller.clone(),
                callee: call.callee.clone(),
                direction: call.direction.clone(),
                start_time: call.start_time,
                answer_time: call.answer_time,
                end_time,
                duration,
                billsec,
                hangup_cause: hangup_cause.clone(),
                server_id: self.server_id.clone(),
            };

            match self.cdr_generator.generate_cdr(hangup_event).await {
                Ok(cdr_id) => {
                    info!("✅ SIMULATOR: CDR {} generated for call {}", cdr_id, call_uuid);
                }
                Err(e) => {
                    error!("❌ SIMULATOR: Failed to generate CDR for {}: {}", call_uuid, e);
                }
            }

            // Update simulation status
            {
                let mut sims = self.active_simulations.write().await;
                if let Some(call) = sims.get_mut(&call_uuid) {
                    call.end_time = Some(end_time);
                    call.status = CallStatus::Completed;
                    call.hangup_cause = Some(hangup_cause);
                }
            }
        }
    }

    async fn remove_active_call(&self, call_uuid: &str) -> Result<(), String> {
        let client = self.db_pool.get().await.map_err(|e| e.to_string())?;

        client.execute(
            "DELETE FROM active_calls WHERE call_id = $1",
            &[&call_uuid],
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    }
}
