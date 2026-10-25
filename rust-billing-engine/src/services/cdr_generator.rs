// src/services/cdr_generator.rs
//
// Writes the CDR of each a-leg at hangup. This SBC node does not bill: CDRs
// carry no account, rate or cost (those columns stay NULL).
use crate::database::DbPool;
use crate::error::EngineError;
use chrono::{DateTime, Utc};
use tracing::{info, error};

pub struct CdrGenerator {
    db_pool: DbPool,
}

#[derive(Debug, Clone)]
pub struct HangupEvent {
    pub uuid: String,
    pub caller: String,
    pub callee: String,
    pub start_time: DateTime<Utc>,
    pub answer_time: Option<DateTime<Utc>>,
    pub end_time: DateTime<Utc>,
    pub duration: i32,
    pub billsec: i32,
    pub hangup_cause: String,
    pub direction: String,
    pub server_id: String,
    /// Post-Dial Delay (INVITE → first 180/183) in ms
    pub pdd_ms: Option<i32>,
}

impl CdrGenerator {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    pub async fn generate_cdr(&self, event: HangupEvent) -> Result<i64, EngineError> {
        let client = self.db_pool.get().await
            .map_err(|e| {
                error!("❌ Failed to get DB connection: {}", e);
                EngineError::Internal(e.to_string())
            })?;

        let row = client
            .query_one(
                "INSERT INTO cdrs
                 (call_uuid, caller_number, called_number, start_time, answer_time, end_time,
                  duration, billsec, hangup_cause, direction, freeswitch_server_id, pdd_ms)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                 RETURNING id",
                &[
                    &event.uuid,
                    &event.caller,
                    &event.callee,
                    &event.start_time,
                    &event.answer_time,
                    &event.end_time,
                    &event.duration,
                    &event.billsec,
                    &event.hangup_cause,
                    &event.direction,
                    &event.server_id,
                    &event.pdd_ms,
                ],
            )
            .await?;

        let cdr_id: i64 = row.get("id");
        info!(
            "✅ CDR generated: ID={}, UUID={}, Direction={}, Duration={}s, Billsec={}s, Cause={}",
            cdr_id, event.uuid, event.direction, event.duration, event.billsec, event.hangup_cause
        );

        Ok(cdr_id)
    }
}
