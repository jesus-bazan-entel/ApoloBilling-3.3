// src/esl/event_handler.rs
//
// ESL events → active_calls tracking and CDRs. This SBC node does no billing:
// calls are never authorized, reserved or rated here, only recorded.
use crate::services::CdrGenerator;
use crate::esl::event::EslEvent;
use crate::database::DbPool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use chrono::Utc;
use tracing::{info, warn, error};

pub struct EventHandler {
    server_id: String,
    cdr_generator: Arc<CdrGenerator>,
    db_pool: DbPool,
    /// a-leg uuid → metrics from its b-legs, held until the a-leg's CDR is
    /// written. The a-leg's own progress is FreeSWITCH's local Ring-Ready
    /// (~20 ms after the INVITE), so the real PDD comes from the b-leg: its
    /// first 180/183 from the destination. Parallel ring keeps the fastest leg.
    pending_bleg: Arc<Mutex<HashMap<String, (LegMetrics, Instant)>>>,
}

/// b-leg metrics whose a-leg never hung up here are dropped after this long
const PENDING_BLEG_TTL: Duration = Duration::from_secs(600);

/// What a call's legs report, merged into its CDR: fastest PDD, worst media,
/// and where the call left the SBC (the answered b-leg, else the first one)
#[derive(Debug, Clone, Default)]
struct LegMetrics {
    pdd_ms: Option<i32>,
    mos: Option<f32>,
    loss_pct: Option<f32>,
    egress: Option<Egress>,
}

/// Outgoing side of the bridge, as FreeSWITCH sent it
#[derive(Debug, Clone, PartialEq)]
pub struct Egress {
    /// User part of the INVITE's To sent to the next hop (`sip_to_user`)
    pub number: String,
    /// Next hop's IP (`sip_network_ip`)
    pub ip: Option<String>,
    /// FreeSWITCH gateway used, if any (`sip_gateway_name`)
    pub gateway: Option<String>,
    answered: bool,
}

impl Egress {
    fn of(event: &EslEvent) -> Option<Self> {
        let number = event
            .get_header("variable_sip_to_user")
            .or_else(|| event.get_header("Caller-Destination-Number"))
            .filter(|n| !n.is_empty())?
            .clone();
        let non_empty = |key: &str| event.get_header(key).filter(|v| !v.is_empty()).cloned();
        Some(Self {
            number,
            ip: non_empty("variable_sip_network_ip"),
            gateway: non_empty("variable_sip_gateway_name"),
            answered: event
                .get_header("variable_answer_epoch")
                .is_some_and(|e| e.trim() != "0" && !e.trim().is_empty()),
        })
    }
}

impl LegMetrics {
    fn of(event: &EslEvent, with_pdd: bool) -> Self {
        let media = event.media_quality();
        Self {
            pdd_ms: if with_pdd { event.pdd_ms() } else { None },
            mos: media.map(|(mos, _)| mos),
            loss_pct: media.map(|(_, loss)| loss),
            egress: None,
        }
    }

    fn is_empty(&self) -> bool {
        self.pdd_ms.is_none() && self.mos.is_none() && self.loss_pct.is_none() && self.egress.is_none()
    }

    fn merge(self, other: Self) -> Self {
        fn pick<T: PartialOrd + Copy>(a: Option<T>, b: Option<T>, keep_a: fn(T, T) -> bool) -> Option<T> {
            match (a, b) {
                (Some(a), Some(b)) => Some(if keep_a(a, b) { a } else { b }),
                (a, b) => a.or(b),
            }
        }
        Self {
            pdd_ms: pick(self.pdd_ms, other.pdd_ms, |a, b| a <= b),
            mos: pick(self.mos, other.mos, |a, b| a <= b),
            loss_pct: pick(self.loss_pct, other.loss_pct, |a, b| a >= b),
            egress: match (self.egress, other.egress) {
                (Some(a), Some(b)) => Some(if b.answered && !a.answered { b } else { a }),
                (a, b) => a.or(b),
            },
        }
    }
}

impl EventHandler {
    pub fn new(server_id: String, cdr_generator: Arc<CdrGenerator>, db_pool: DbPool) -> Self {
        Self {
            server_id,
            cdr_generator,
            db_pool,
            pending_bleg: Arc::default(),
        }
    }

    pub fn new_server_mode(cdr_generator: Arc<CdrGenerator>, db_pool: DbPool) -> Self {
        Self::new("esl-server".to_string(), cdr_generator, db_pool)
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

        let (direction, context) = sbc_direction(event);
        info!("📞 Context: {} → Direction: {}", context, direction);

        info!("📞 CHANNEL_CREATE (a-leg): {} - {} → {} [{}]", uuid, caller, callee, direction);

        // Track the call; nothing is authorized or reserved on this node
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

    /// Attach a b-leg's PDD and media quality to its a-leg: kept in memory if
    /// the a-leg CDR is not written yet, otherwise applied to the CDR directly
    /// (LEAST/GREATEST ignore NULLs: fastest PDD, worst MOS and loss win).
    async fn record_bleg_metrics(&self, event: &EslEvent) {
        let metrics = LegMetrics {
            egress: Egress::of(event),
            ..LegMetrics::of(event, true)
        };
        let Some(a_uuid) = event
            .get_header("Other-Leg-Unique-ID")
            .or_else(|| event.get_header("variable_originating_leg_uuid"))
            .cloned()
        else {
            return;
        };
        if metrics.is_empty() {
            return;
        }

        // The a-leg's CDR may already be written: the answered b-leg (or the
        // first one, if none answered) sets its egress
        let egress = metrics.egress.as_ref();
        let (eg_number, eg_ip, eg_gateway, eg_answered) = (
            egress.map(|e| e.number.clone()),
            egress.and_then(|e| e.ip.clone()),
            egress.and_then(|e| e.gateway.clone()),
            egress.is_some_and(|e| e.answered),
        );
        if let Ok(client) = self.db_pool.get().await {
            match client
                .execute(
                    "UPDATE cdrs SET pdd_ms = LEAST(pdd_ms, $2), mos = LEAST(mos, $3),
                                     rtp_loss_pct = GREATEST(rtp_loss_pct, $4),
                                     egress_number = CASE WHEN $5::text IS NOT NULL
                                         AND (egress_number IS NULL OR $8) THEN $5 ELSE egress_number END,
                                     egress_ip = CASE WHEN $5::text IS NOT NULL
                                         AND (egress_number IS NULL OR $8) THEN $6 ELSE egress_ip END,
                                     egress_gateway = CASE WHEN $5::text IS NOT NULL
                                         AND (egress_number IS NULL OR $8) THEN $7 ELSE egress_gateway END
                     WHERE call_uuid = $1 AND start_time > now() - INTERVAL '1 day'",
                    &[&a_uuid, &metrics.pdd_ms, &metrics.mos, &metrics.loss_pct,
                      &eg_number, &eg_ip, &eg_gateway, &eg_answered],
                )
                .await
            {
                Ok(n) if n > 0 => return,
                Ok(_) => {}
                Err(e) => warn!("Could not store b-leg metrics for {}: {}", a_uuid, e),
            }
        }

        let mut pending = self.pending_bleg.lock().unwrap_or_else(|e| e.into_inner());
        pending.retain(|_, (_, at)| at.elapsed() < PENDING_BLEG_TTL);
        let entry = pending
            .entry(a_uuid)
            .or_insert_with(|| (LegMetrics::default(), Instant::now()));
        entry.0 = std::mem::take(&mut entry.0).merge(metrics);
    }

    fn take_pending_bleg(&self, a_uuid: &str) -> LegMetrics {
        self.pending_bleg
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(a_uuid)
            .map(|(m, _)| m)
            .unwrap_or_default()
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
            self.record_bleg_metrics(event).await;
            return;
        }

        let caller = event.caller().cloned().unwrap_or_default();
        let callee = event.callee().cloned().unwrap_or_default();
        let duration = event.duration().unwrap_or(0);
        let billsec = event.billsec().unwrap_or(0);
        let hangup_cause = event.hangup_cause().cloned().unwrap_or_else(|| "UNKNOWN".to_string());

        // A destination that wasn't registered can only be the called party
        let direction = if hangup_cause == "USER_NOT_REGISTERED" {
            "inbound".to_string()
        } else {
            sbc_direction(event).0
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

        // The a-leg's own progress is local Ring-Ready (not a PDD); its media
        // stats are what FreeSWITCH received from the caller's side
        let legs = self.take_pending_bleg(&uuid).merge(LegMetrics::of(event, false));

        // Generate CDR
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
            transformed_number: event.get_header("variable_transformed_number").cloned(),
            sip_code: final_sip_code(event),
            source_ip: event.get_header("variable_sip_network_ip").cloned(),
            egress: legs.egress.clone(),
            pdd_ms: legs.pdd_ms,
            mos: legs.mos,
            rtp_loss_pct: legs.loss_pct,
        };

        if let Err(e) = self.cdr_generator.generate_cdr(hangup_event).await {
            error!("Error generating CDR for {}: {}", uuid, e);
        }
    }
}

/// Final SIP status the caller received. `sip_term_status` is only set when
/// FreeSWITCH itself sent the final reply; otherwise an answered a-leg got a
/// 200, and an unanswered one the `sip:NNN` of its own or its bridged leg's
/// protocol-specific cause (what FreeSWITCH relayed).
fn final_sip_code(event: &EslEvent) -> Option<i16> {
    let parse = |key: &str| -> Option<i16> {
        let v = event.get_header(key)?;
        v.trim().trim_start_matches("sip:").parse().ok()
    };
    parse("variable_sip_term_status")
        .or_else(|| event.get_header("variable_answer_epoch")
            .filter(|e| e.trim() != "0" && !e.trim().is_empty())
            .map(|_| 200))
        .or_else(|| parse("variable_proto_specific_hangup_cause"))
        .or_else(|| parse("variable_last_bridge_proto_specific_hangup_cause"))
        .or_else(|| {
            // FreeSWITCH refused the call itself (e.g. the bridge failed and
            // the dialplan ended): it answered with mod_sofia's fixed
            // hangup_cause_to_sip mapping and set no sip_term_status
            let refused = event
                .get_header("variable_sip_hangup_disposition")
                .is_some_and(|d| d == "send_refuse");
            refused.then(|| event.hangup_cause().and_then(|c| sofia_refuse_code(c))).flatten()
        })
}

/// mod_sofia `hangup_cause_to_sip` for the causes seen on this SBC (its
/// default for anything else is 480)
fn sofia_refuse_code(cause: &str) -> Option<i16> {
    Some(match cause {
        "UNALLOCATED_NUMBER" | "NO_ROUTE_TRANSIT_NET" | "NO_ROUTE_DESTINATION" => 404,
        "USER_BUSY" => 486,
        "NO_USER_RESPONSE" | "NO_ANSWER" | "SUBSCRIBER_ABSENT" | "USER_NOT_REGISTERED" => 480,
        "CALL_REJECTED" => 603,
        "NUMBER_CHANGED" | "REDIRECTION_TO_NEW_DESTINATION" => 410,
        "DESTINATION_OUT_OF_ORDER" | "NETWORK_OUT_OF_ORDER" => 502,
        "INVALID_NUMBER_FORMAT" => 484,
        "NORMAL_TEMPORARY_FAILURE" | "SWITCH_CONGESTION" | "NORMAL_CIRCUIT_CONGESTION"
        | "REQUESTED_CHAN_UNAVAIL" | "SERVICE_UNAVAILABLE" => 503,
        "RECOVERY_ON_TIMER_EXPIRE" => 504,
        "INCOMPATIBLE_DESTINATION" | "BEARERCAPABILITY_NOTAVAIL" => 488,
        "ORIGINATOR_CANCEL" => 487,
        _ => return None,
    })
}

/// Business direction of an a-leg, plus its dialplan context for logging.
///
/// FreeSWITCH's own `Call-Direction` is relative to FreeSWITCH, so every
/// a-leg reports "inbound" (on 2026-10-25 100% of CDRs, including app users
/// calling mobiles for 53 min). Neither the context nor the profile is enough
/// either: FS Legado (10.118.0.4) is in the internal profile's ACL, so its
/// calls to app users also land in `from-pbx` on `sofia/internal` (5,208 of
/// 5,380 such a-legs 2026-10-17..25). What only a customer's device has is a
/// directory-authenticated INVITE: mod_xml_curl returns its `accountcode`
/// (= account id), which ACL-trusted peers never get. So outbound = handled
/// in `from-pbx` with an `accountcode`; everything else is inbound.
fn sbc_direction(event: &EslEvent) -> (String, String) {
    let context = event
        .get_header("Caller-Context")
        .or_else(|| event.get_header("variable_context"))
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    let authenticated_device = event
        .get_header("variable_accountcode")
        .is_some_and(|a| !a.trim().is_empty());

    let direction = if context.starts_with("from-pbx") && authenticated_device {
        "outbound"
    } else {
        "inbound"
    };
    (direction.to_string(), context)
}

#[cfg(test)]
mod direction_tests {
    use super::*;

    fn leg(channel: &str, context: &str, accountcode: &str) -> EslEvent {
        let headers = [
            ("Caller-Channel-Name", channel),
            ("Caller-Context", context),
            ("Call-Direction", "inbound"),
            ("variable_accountcode", accountcode),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        EslEvent { headers, ..Default::default() }
    }

    #[test]
    fn sip_code_fallbacks() {
        let ev = |pairs: &[(&str, &str)]| EslEvent {
            headers: pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            ..Default::default()
        };
        assert_eq!(final_sip_code(&ev(&[("variable_sip_term_status", "487")])), Some(487));
        assert_eq!(final_sip_code(&ev(&[("variable_answer_epoch", "1761430000")])), Some(200));
        assert_eq!(
            final_sip_code(&ev(&[
                ("variable_answer_epoch", "0"),
                ("variable_last_bridge_proto_specific_hangup_cause", "sip:486"),
            ])),
            Some(486)
        );
        assert_eq!(
            final_sip_code(&ev(&[
                ("variable_sip_hangup_disposition", "send_refuse"),
                ("Hangup-Cause", "CALL_REJECTED"),
            ])),
            Some(603)
        );
        assert_eq!(final_sip_code(&ev(&[])), None);
    }

    #[test]
    fn app_device_is_outbound() {
        let ev = leg("sofia/internal/17486128@app.fonowin.pe", "from-pbx", "1172");
        assert_eq!(sbc_direction(&ev).0, "outbound");
    }

    #[test]
    fn carrier_and_legado_are_inbound() {
        let ev = leg("sofia/external/939513001@10.35.12.68", "from-kamailio", "");
        assert_eq!(sbc_direction(&ev).0, "inbound");
        // Legado is ACL-trusted on the internal profile: from-pbx, no accountcode
        let ev = leg("sofia/internal/957185999@10.118.0.4", "from-pbx", "");
        assert_eq!(sbc_direction(&ev).0, "inbound");
        assert_eq!(sbc_direction(&EslEvent::default()).0, "inbound");
    }
}

#[cfg(test)]
mod egress_tests {
    use super::*;

    fn bleg(to: &str, ip: &str, gw: &str, answer_epoch: &str) -> EslEvent {
        let headers = [
            ("variable_sip_to_user", to),
            ("variable_sip_network_ip", ip),
            ("variable_sip_gateway_name", gw),
            ("variable_answer_epoch", answer_epoch),
        ]
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        EslEvent { headers, ..Default::default() }
    }

    fn legs(ev: &EslEvent) -> LegMetrics {
        LegMetrics { egress: Egress::of(ev), ..Default::default() }
    }

    #[test]
    fn egress_from_bleg() {
        let e = Egress::of(&bleg("19020370", "10.118.0.4", "freeswitch-legado", "1792972133")).unwrap();
        assert_eq!(e.number, "19020370");
        assert_eq!(e.ip.as_deref(), Some("10.118.0.4"));
        assert_eq!(e.gateway.as_deref(), Some("freeswitch-legado"));
        assert!(Egress::of(&EslEvent::default()).is_none());
    }

    #[test]
    fn parallel_ring_keeps_the_answered_leg() {
        let legado = legs(&bleg("17473046", "10.118.0.4", "freeswitch-legado", "0"));
        let app = legs(&bleg("17473046", "45.5.68.242", "", "1792972133"));
        let merged = legado.clone().merge(app.clone());
        assert_eq!(merged.egress.unwrap().ip.as_deref(), Some("45.5.68.242"));
        // Nobody answered: the first leg stays
        let unanswered = legs(&bleg("17473046", "45.5.68.242", "", "0"));
        assert_eq!(legado.merge(unanswered).egress.unwrap().ip.as_deref(), Some("10.118.0.4"));
    }
}
