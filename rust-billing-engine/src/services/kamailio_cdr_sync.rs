// src/services/kamailio_cdr_sync.rs
//
// Imports Kamailio dialog CDRs (kamailio.acc_cdrs) into PostgreSQL `cdrs`.
//
// Inbound DIDs whose route is "direct" go Carrier -> Kamailio -> trunk without
// touching FreeSWITCH, so no ESL events reach CdrGenerator. kamailio.cfg tags
// those dialogs via='direct' (acc cdr_extra); everything that still crosses
// FreeSWITCH is tagged via='fs' and already gets its CDR from ESL, so it is
// skipped here (kept in acc_cdrs only for cross-checking).
//
// The PG insert and the cursor advance (kamailio_cdr_sync_state) commit in the
// same transaction, so each acc_cdrs row is imported exactly once.
use crate::database::DbPool;
use crate::services::cdr_generator::{customer_account_sql, decode_sip_user};
use crate::services::numbering::{customer_keys, routed_number, to_e164};
use chrono::{DateTime, TimeZone, Utc};
use sqlx::mysql::{MySqlConnectOptions, MySqlPool, MySqlPoolOptions};
use std::str::FromStr;
use sqlx::Row;
use std::time::Duration;
use tracing::{error, info, warn};

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const BATCH_SIZE: i64 = 500;
const SERVER_ID: &str = "kamailio";

pub struct KamailioCdrSync {
    db_pool: DbPool,
    mysql_pool: MySqlPool,
}

#[derive(Debug)]
struct AccCdr {
    id: i64,
    callid: String,
    src_user: String,
    dst_user: String,
    via: String,
    start_ts: i64,
    end_ts: i64,
    answer_ts: i64,
    final_ts: i64,
    sip_code: String,
}

impl KamailioCdrSync {
    pub async fn connect(db_pool: DbPool, kamailio_url: &str) -> Result<Self, sqlx::Error> {
        // sqlx sets time_zone='+00:00' on every MySQL session by default, which
        // makes UNIX_TIMESTAMP() read acc's local DATETIMEs as UTC (CDRs 5h off).
        // timezone(None) keeps the server's own zone, the one acc wrote them in.
        let options = MySqlConnectOptions::from_str(kamailio_url)?.timezone(None);
        let mysql_pool = MySqlPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(options)
            .await?;
        Ok(Self { db_pool, mysql_pool })
    }

    pub async fn run(self) {
        info!("✅ Kamailio CDR sync started (acc_cdrs via=direct -> cdrs)");
        loop {
            match self.sync_batch().await {
                // A full batch means there is a backlog: keep going without sleeping.
                Ok(n) if n as i64 == BATCH_SIZE => continue,
                Ok(_) => {}
                Err(e) => error!("Kamailio CDR sync error: {}", e),
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    /// Returns how many acc_cdrs rows were consumed (imported or skipped).
    async fn sync_batch(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let mut client = self.db_pool.get().await?;
        let cursor: i64 = client
            .query_one(
                "SELECT last_acc_cdr_id FROM kamailio_cdr_sync_state WHERE id = 1",
                &[],
            )
            .await?
            .get(0);

        // UNIX_TIMESTAMP() converts acc's local DATETIME using MariaDB's own
        // time zone, so we never have to guess the server offset here.
        let rows = sqlx::query(
            "SELECT id, callid, src_user, dst_user, via, \
                    CAST(UNIX_TIMESTAMP(start_time) AS SIGNED) AS start_ts, \
                    CAST(UNIX_TIMESTAMP(end_time) AS SIGNED) AS end_ts, \
                    answer_ts, final_ts, sip_code \
             FROM acc_cdrs WHERE id > ? ORDER BY id LIMIT ?",
        )
        .bind(cursor)
        .bind(BATCH_SIZE)
        .fetch_all(&self.mysql_pool)
        .await?;

        if rows.is_empty() {
            return Ok(0);
        }

        let cdrs: Vec<AccCdr> = rows
            .iter()
            .map(|r| AccCdr {
                id: r.get::<u32, _>("id") as i64,
                callid: r.get("callid"),
                src_user: r.get("src_user"),
                dst_user: r.get("dst_user"),
                via: r.get("via"),
                start_ts: r.get("start_ts"),
                end_ts: r.get("end_ts"),
                // "" when the call was never answered
                answer_ts: r.get::<String, _>("answer_ts").parse().unwrap_or(0),
                final_ts: r.get::<String, _>("final_ts").parse().unwrap_or(0),
                sip_code: r.get("sip_code"),
            })
            .collect();
        let last_id = cdrs.last().map(|c| c.id).unwrap_or(cursor);

        let tx = client.transaction().await?;
        let mut imported = 0;
        for cdr in cdrs.iter().filter(|c| c.via == "direct") {
            if cdr.callid.is_empty() {
                warn!("acc_cdrs id={} has no callid - skipped", cdr.id);
                continue;
            }
            let answered = cdr.answer_ts > 0;
            // acc leaves end_time = start_time on failed dialogs; the cfg stores
            // the final reply time separately so ring time isn't lost.
            let end_ts = if !answered && cdr.final_ts > cdr.end_ts {
                cdr.final_ts
            } else {
                cdr.end_ts
            };
            let start_time = to_utc(cdr.start_ts);
            let end_time = to_utc(end_ts.max(cdr.start_ts));
            let answer_time = answered.then(|| to_utc(cdr.answer_ts));
            let duration = (end_ts - cdr.start_ts).max(0) as i32;
            let billsec = if answered {
                (end_ts - cdr.answer_ts).max(0) as i32
            } else {
                0
            };
            let hangup_cause = if answered {
                "NORMAL_CLEARING"
            } else {
                sip_code_to_cause(&cdr.sip_code)
            };
            let call_uuid: String = cdr.callid.chars().take(100).collect();
            let caller = decode_sip_user(&cdr.src_user);
            let callee = decode_sip_user(&cdr.dst_user);
            let routed = routed_number(&callee, None);
            let destination = to_e164(&routed);
            let customer = customer_keys(&caller, &routed, "inbound");

            imported += tx
                .execute(
                    &format!(
                        "INSERT INTO cdrs
                         (call_uuid, caller_number, called_number, start_time, answer_time,
                          end_time, duration, billsec, hangup_cause, direction,
                          freeswitch_server_id, destination_prefix, account_id)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'inbound', $10, $11, {})
                         ON CONFLICT (call_uuid, start_time) DO NOTHING",
                        customer_account_sql(12)
                    ),
                    &[
                        &call_uuid,
                        &caller,
                        &callee,
                        &start_time,
                        &answer_time,
                        &end_time,
                        &duration,
                        &billsec,
                        &hangup_cause,
                        &SERVER_ID,
                        &destination,
                        &customer,
                    ],
                )
                .await?;
        }
        tx.execute(
            "UPDATE kamailio_cdr_sync_state SET last_acc_cdr_id = $1, updated_at = now() WHERE id = 1",
            &[&last_id],
        )
        .await?;
        tx.commit().await?;

        if imported > 0 {
            info!(
                "Kamailio CDR sync: imported {} direct CDRs (cursor {} -> {})",
                imported, cursor, last_id
            );
        }
        Ok(cdrs.len())
    }
}

fn to_utc(ts: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(ts, 0).single().unwrap_or_else(Utc::now)
}

/// Same mapping FreeSWITCH applies to SIP finals (sofia sip2cause), so direct
/// CDRs read like the ESL-generated ones in reports.
fn sip_code_to_cause(code: &str) -> &'static str {
    match code {
        "404" | "485" | "604" => "UNALLOCATED_NUMBER",
        "401" | "403" | "407" | "603" => "CALL_REJECTED",
        "408" | "504" => "RECOVERY_ON_TIMER_EXPIRE",
        "410" => "NUMBER_CHANGED",
        "480" => "NO_USER_RESPONSE",
        "484" => "INVALID_NUMBER_FORMAT",
        "486" | "600" => "USER_BUSY",
        "487" => "ORIGINATOR_CANCEL",
        "488" | "606" => "INCOMPATIBLE_DESTINATION",
        "500" | "503" => "NORMAL_TEMPORARY_FAILURE",
        "501" => "SERVICE_NOT_IMPLEMENTED",
        "502" => "NETWORK_OUT_OF_ORDER",
        _ => "NORMAL_UNSPECIFIED",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_sip_finals() {
        assert_eq!(sip_code_to_cause("487"), "ORIGINATOR_CANCEL");
        assert_eq!(sip_code_to_cause("486"), "USER_BUSY");
        assert_eq!(sip_code_to_cause("401"), "CALL_REJECTED");
        assert_eq!(sip_code_to_cause("408"), "RECOVERY_ON_TIMER_EXPIRE");
        assert_eq!(sip_code_to_cause(""), "NORMAL_UNSPECIFIED");
    }
}
