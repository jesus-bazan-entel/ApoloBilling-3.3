// src/services/cdr_generator.rs
//
// Writes the CDR of each a-leg at hangup. This SBC node does not bill: rate and
// cost stay NULL. `account_id` is the customer (Cliente) whose SIP device is
// the routed destination (inbound) or the caller (outbound), NULL when the
// number belongs to no device (e.g. FS Legado customers).
use crate::database::DbPool;
use crate::error::EngineError;
use crate::services::numbering::{caller_to_e164, customer_keys, routed_number, to_e164};
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
    /// FreeSWITCH `transformed_number` (inbound route strip/prefix), if set
    pub transformed_number: Option<String>,
    /// Final SIP status sent on the a-leg (`sip_term_status`)
    pub sip_code: Option<i16>,
    /// IP that sent the INVITE (`sip_network_ip`)
    pub source_ip: Option<String>,
    /// Where the call left the SBC (b-leg), if a b-leg was seen already
    pub egress: Option<crate::esl::event_handler::Egress>,
    /// Post-Dial Delay (INVITE → first 180/183) in ms
    pub pdd_ms: Option<i32>,
    /// Worst leg's FreeSWITCH-estimated MOS and received-RTP loss percent
    pub mos: Option<f32>,
    pub rtp_loss_pct: Option<f32>,
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

        let caller = decode_sip_user(&event.caller);
        let callee = decode_sip_user(&event.callee);
        let routed = routed_number(&callee, event.transformed_number.as_deref());
        let destination = to_e164(&routed);
        let customer = customer_keys(&caller, &routed, &event.direction);

        let row = client
            .query_one(
                &format!(
                    "INSERT INTO cdrs
                     (call_uuid, caller_number, called_number, start_time, answer_time, end_time,
                      duration, billsec, hangup_cause, direction, freeswitch_server_id, pdd_ms,
                      mos, rtp_loss_pct, destination_prefix, account_id, caller_e164, sip_code,
                      source_ip, egress_number, egress_ip, egress_gateway)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                             {}, $17, $18, $19, $20, $21, $22)
                     RETURNING id",
                    customer_account_sql(16)
                ),
                &[
                    &event.uuid,
                    &caller,
                    &callee,
                    &event.start_time,
                    &event.answer_time,
                    &event.end_time,
                    &event.duration,
                    &event.billsec,
                    &event.hangup_cause,
                    &event.direction,
                    &event.server_id,
                    &event.pdd_ms,
                    &event.mos,
                    &event.rtp_loss_pct,
                    &destination,
                    &customer,
                    &caller_to_e164(&caller),
                    &event.sip_code,
                    &event.source_ip,
                    &event.egress.as_ref().map(|e| e.number.clone()),
                    &event.egress.as_ref().and_then(|e| e.ip.clone()),
                    &event.egress.as_ref().and_then(|e| e.gateway.clone()),
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

/// Subquery resolving `account_id` from a text[] parameter of
/// `sip_devices.sip_username` candidates (see `numbering::customer_keys`)
pub fn customer_account_sql(param: usize) -> String {
    format!(
        "(SELECT account_id FROM sip_devices WHERE sip_username = ANY(${}::text[]) \
         ORDER BY enabled DESC, id LIMIT 1)",
        param
    )
}

/// Percent-decode a SIP URI user part. Carriers send E.164 callers as
/// `%2B1850…` in the From URI and FreeSWITCH passes them through verbatim, so
/// CDRs stored `%2B` instead of `+`. Invalid escapes, control characters
/// (`%0A`…) and non-UTF-8 results are kept as received.
pub fn decode_sip_user(user: &str) -> String {
    if !user.contains('%') {
        return user.to_string();
    }
    let bytes = user.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            let decoded = hex.and_then(|h| u8::from_str_radix(h, 16).ok());
            if let Some(b) = decoded.filter(|b| !b.is_ascii_control()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| user.to_string())
}

#[cfg(test)]
mod decode_tests {
    use super::decode_sip_user;

    #[test]
    fn decodes_percent_escapes() {
        assert_eq!(decode_sip_user("%2B18505551234"), "+18505551234");
        assert_eq!(decode_sip_user("912345678"), "912345678");
        assert_eq!(decode_sip_user("%2b1"), "+1");
        // Broken escapes are left alone
        assert_eq!(decode_sip_user("50%"), "50%");
        assert_eq!(decode_sip_user("%ZZ12"), "%ZZ12");
        assert_eq!(decode_sip_user("%2"), "%2");
        // Control characters stay escaped
        assert_eq!(decode_sip_user("(01)%20215-8100%0A"), "(01) 215-8100%0A");
    }
}
