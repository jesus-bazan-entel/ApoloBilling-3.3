// src/services/cdr_generator.rs
use crate::database::DbPool;
use crate::services::minute_bucket_manager::{ConsumeOutcome, MinuteBucketManager};
use crate::error::BillingError;
use std::sync::Arc;
use chrono::{DateTime, Utc};

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use tracing::{info, error};

/// Billing increment (seconds) used to round minute-bucket calls. Bucket
/// minute counting is independent of any matched `rate_cards` row (a bucket
/// call may never touch a rate card at all if it stays fully in-bucket), so
/// it always uses this fixed default rather than a per-destination increment.
const BUCKET_BILLING_INCREMENT: i32 = 6;

pub struct CdrGenerator {
    db_pool: DbPool,
    minute_bucket_mgr: Arc<MinuteBucketManager>,
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
    pub source_ip: Option<String>,
}

impl CdrGenerator {
    pub fn new(db_pool: DbPool, minute_bucket_mgr: Arc<MinuteBucketManager>) -> Self {
        Self {
            db_pool,
            minute_bucket_mgr,
        }
    }

    pub async fn generate_cdr(&self, event: HangupEvent) -> Result<i64, BillingError> {
        let is_inbound = event.direction.to_lowercase() == "inbound";

        info!(
            "📝 Generating CDR for call: {} [{}]",
            event.uuid,
            if is_inbound { "INBOUND - No billing" } else { "OUTBOUND - Billable" }
        );

        // Every outbound call that was actually authorized left a bucket hold
        // in Redis; `consume()` returns NotApplicable for inbound calls and
        // for outbound calls that skipped authorization entirely (the
        // `require_outbound_authorization=false` system setting) — those
        // still get a best-effort LPM cost lookup below, but never touch a
        // bucket cycle (there was nothing to reserve against).
        let bucket_outcome = if is_inbound {
            ConsumeOutcome::NotApplicable
        } else {
            let rounded_billsec_bucket = if event.billsec > 0 {
                ((event.billsec + BUCKET_BILLING_INCREMENT - 1) / BUCKET_BILLING_INCREMENT)
                    * BUCKET_BILLING_INCREMENT
            } else {
                event.billsec
            };
            let billed_minutes = Decimal::from(rounded_billsec_bucket) / Decimal::from(60);

            self.minute_bucket_mgr
                .consume(&event.uuid, billed_minutes, &event.callee)
                .await?
        };

        let client = self.db_pool.get().await
            .map_err(|e| {
                error!("❌ Failed to get DB connection: {}", e);
                BillingError::Internal(e.to_string())
            })?;

        // Bucket-specific CDR columns (NULL for calls that never touched a bucket)
        let (bucket_cycle_id, in_bucket_minutes, bucket_overage_minutes, bucket_overage_cost, overage_rate_applied):
            (Option<i32>, Option<Decimal>, Option<Decimal>, Option<Decimal>, Option<Decimal>) =
            if let ConsumeOutcome::Consumed(ref r) = bucket_outcome {
                (
                    Some(r.bucket_cycle_id),
                    Some(r.in_bucket_minutes),
                    Some(r.overage_minutes),
                    Some(r.overage_cost),
                    r.overage_rate_applied,
                )
            } else {
                (None, None, None, None, None)
            };

        let account_id: Option<i32>;
        let rate_per_minute: Option<Decimal>;
        let cost: Option<Decimal>;

        if let ConsumeOutcome::Consumed(ref r) = bucket_outcome {
            // Bucket call: cost to the customer is only the overage portion
            // (in-bucket minutes are $0 — already paid for via the bundle).
            account_id = Some(r.account_id);
            rate_per_minute = r.overage_rate_applied;
            cost = Some(r.overage_cost);

            info!(
                "💰 Bucket call: {} min in-bucket, {} min overage (${} @ {:?}/min)",
                r.in_bucket_minutes, r.overage_minutes, r.overage_cost, r.overage_rate_applied
            );
        } else if is_inbound {
            info!("📞 INBOUND call {} - No billing required", event.uuid);
            account_id = None;
            rate_per_minute = None;
            cost = None;
        } else {
            // OUTBOUND call that never touched a bucket (authorization
            // disabled system-wide). Still calculate cost via LPM rate
            // lookup so the CDR isn't silently free.
            info!("📊 No bucket hold for OUTBOUND call {}, performing LPM rate lookup for billing", event.uuid);
            let (acc_id, rate, c) = self.lookup_rate_and_calculate_cost(&event).await;
            account_id = acc_id;
            rate_per_minute = rate;
            cost = c;
        }

        // TIMESTAMP WITH TIME ZONE acepta DateTime<Utc> directamente
        let start_time = event.start_time;
        let answer_time = event.answer_time;
        let end_time = event.end_time;

        // Insert CDR - compatible con esquema actual + columnas de bolsa de minutos
        let row = client
            .query_one(
                "INSERT INTO cdrs
                 (call_uuid, account_id, caller_number, called_number, start_time, answer_time, end_time,
                  duration, billsec, hangup_cause, rate_per_minute, cost, direction, freeswitch_server_id,
                  bucket_cycle_id, in_bucket_minutes, overage_minutes, overage_cost, overage_rate_applied)
                 VALUES ($1, $2, $3, $4, $5, $6, $7,
                         $8, $9, $10, $11, $12, $13, $14,
                         $15, $16, $17, $18, $19)
                 RETURNING id",
                &[
                    &event.uuid,
                    &account_id,
                    &event.caller,
                    &event.callee,
                    &start_time,
                    &answer_time,
                    &end_time,
                    &event.duration,
                    &event.billsec,
                    &event.hangup_cause,
                    &rate_per_minute,
                    &cost,
                    &event.direction,
                    &event.server_id,
                    &bucket_cycle_id,
                    &in_bucket_minutes,
                    &bucket_overage_minutes,
                    &bucket_overage_cost,
                    &overage_rate_applied,
                ],
            )
            .await?;

        let cdr_id: i64 = row.get("id");

        info!(
            "✅ CDR generated: ID={}, UUID={}, Direction={}, Duration={}s, Billsec={}s, Cost=${:?}, Cause={}",
            cdr_id, event.uuid, event.direction, event.duration, event.billsec,
            cost.map(|c| c.to_f64().unwrap_or(0.0)),
            event.hangup_cause
        );

        Ok(cdr_id)
    }

    /// Lookup rate via LPM and calculate cost for outbound calls that never
    /// touched a bucket (e.g. `require_outbound_authorization=false`).
    /// Also finds account via sip_endpoints (username or IP) for proper CDR linking.
    async fn lookup_rate_and_calculate_cost(
        &self,
        event: &HangupEvent,
    ) -> (Option<i32>, Option<Decimal>, Option<Decimal>) {
        let client = match self.db_pool.get().await {
            Ok(c) => c,
            Err(e) => {
                error!("❌ Failed to get DB connection for rate lookup: {}", e);
                return (None, None, None);
            }
        };

        // 1. Find account by caller number, with sip_endpoints fallback (username and IP)
        let normalized_caller = event.caller.replace('+', "").replace(' ', "").replace('-', "");
        let source_ip = event.source_ip.as_deref();

        // Try direct account_number match
        let mut account_id: Option<i32> = match client.query_opt(
            "SELECT id FROM accounts WHERE (account_number = $1 OR account_number = $2) LIMIT 1",
            &[&event.caller, &normalized_caller],
        ).await {
            Ok(Some(row)) => {
                let id: i32 = row.get(0);
                info!("📊 Found account {} for caller {} (direct match)", id, event.caller);
                Some(id)
            }
            Ok(None) => None,
            Err(e) => {
                error!("❌ Error looking up account for caller {}: {}", event.caller, e);
                None
            }
        };

        // Fallback: look up by sip_username (credentials auth)
        if account_id.is_none() {
            account_id = match client.query_opt(
                "SELECT a.id FROM sip_endpoints se
                 JOIN accounts a ON a.id = se.account_id
                 WHERE se.sip_username = $1
                 LIMIT 1",
                &[&event.caller],
            ).await {
                Ok(Some(row)) => {
                    let id: i32 = row.get(0);
                    info!("📊 Found account {} for caller {} (via sip_username)", id, event.caller);
                    Some(id)
                }
                Ok(None) => None,
                Err(e) => {
                    error!("❌ Error looking up account via sip_username: {}", e);
                    None
                }
            };
        }

        // For IP-authenticated trunks, look up by source_ip
        if account_id.is_none() {
            if let Some(ip) = source_ip {
                account_id = match client.query_opt(
                    "SELECT a.id FROM sip_endpoints se
                     JOIN accounts a ON a.id = se.account_id
                     WHERE se.host = $1
                     LIMIT 1",
                    &[&ip],
                ).await {
                    Ok(Some(row)) => {
                        let id: i32 = row.get(0);
                        info!("📊 Found account {} for caller {} (via source_ip {})", id, event.caller, ip);
                        Some(id)
                    }
                    Ok(None) => None,
                    Err(e) => {
                        error!("❌ Error looking up account via source_ip: {}", e);
                        None
                    }
                };
            }
        }

        if account_id.is_none() {
            info!("ℹ️  No account found for caller {} (normalized: {}) source_ip: {:?}", event.caller, normalized_caller, source_ip);
        }

        // 2. LPM rate lookup for destination
        let normalized_dest = event.callee.replace('+', "");
        let mut prefixes: Vec<String> = Vec::new();
        for i in (1..=normalized_dest.len()).rev() {
            prefixes.push(normalized_dest[..i].to_string());
        }

        let rate_row = match client.query_opt(
            "SELECT rate_per_minute, billing_increment, destination_name
             FROM rate_cards
             WHERE destination_prefix = ANY($1)
             AND card_group = 'standard'
             AND effective_start <= NOW()
             AND (effective_end IS NULL OR effective_end >= NOW())
             ORDER BY LENGTH(destination_prefix) DESC, priority DESC
             LIMIT 1",
            &[&prefixes],
        ).await {
            Ok(row) => row,
            Err(e) => {
                error!("❌ Error looking up rate for destination {}: {}", event.callee, e);
                return (account_id, None, None);
            }
        };

        if let Some(row) = rate_row {
            let rate: Decimal = row.get(0);
            let increment: i32 = row.try_get::<_, Option<i32>>(1).unwrap_or(Some(6)).unwrap_or(6);
            let dest_name: String = row.get(2);

            // Calculate cost with billing increment
            let rounded_billsec = if increment > 0 && event.billsec > 0 {
                ((event.billsec + increment - 1) / increment) * increment
            } else {
                event.billsec
            };

            let minutes = Decimal::from(rounded_billsec) / Decimal::from(60);
            let cost = minutes * rate;

            info!(
                "💰 LPM billing (no auth): {} → {} ({}), {}s → {}s ({}s increment) = {} min × ${}/min = ${}",
                event.caller, event.callee, dest_name,
                event.billsec, rounded_billsec, increment, minutes, rate, cost
            );

            (account_id, Some(rate), Some(cost))
        } else {
            info!("ℹ️  No rate found for destination {} - CDR without cost", event.callee);
            (account_id, None, None)
        }
    }
}
