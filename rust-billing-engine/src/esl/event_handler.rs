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

/// What a call's legs report, merged into its CDR: fastest PDD, worst media
#[derive(Debug, Clone, Copy, Default)]
struct LegMetrics {
    pdd_ms: Option<i32>,
    mos: Option<f32>,
    loss_pct: Option<f32>,
}

impl LegMetrics {
    fn of(event: &EslEvent, with_pdd: bool) -> Self {
        let media = event.media_quality();
        Self {
            pdd_ms: if with_pdd { event.pdd_ms() } else { None },
            mos: media.map(|(mos, _)| mos),
            loss_pct: media.map(|(_, loss)| loss),
        }
    }

    fn is_empty(&self) -> bool {
        self.pdd_ms.is_none() && self.mos.is_none() && self.loss_pct.is_none()
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

        // 🔍 Detectar dirección de la llamada (inbound/outbound)
        //
        // Fix 2026-08-09 (soporte, revisión completa): "context" (from-pbx/
        // from-kamailio) NO es confiable - describe qué perfil de FreeSWITCH
        // está *entregando* la llamada en este momento, no quién la originó
        // (confirmado con volcado de headers en vivo: llamadas de un móvil
        // externo hacia una extensión no registrada reportan
        // Caller-Context=from-pbx igual que un usuario interno saliendo).
        //
        // FreeSWITCH SÍ trae la dirección real y confiable en headers nativos
        // (Call-Direction / Caller-Direction / Caller-Logical-Direction /
        // variable_direction), presentes en prácticamente todas las llamadas
        // - solo que el código nunca los miraba primero. Se usan como señal
        // principal; "context" queda solo de último recurso si ninguno viene.
        let context = event.get_header("Caller-Context")
            .or_else(|| event.get_header("variable_context"))
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        let native_direction = event.get_header("Call-Direction")
            .or_else(|| event.get_header("Caller-Direction"))
            .or_else(|| event.get_header("Caller-Logical-Direction"))
            .or_else(|| event.get_header("variable_direction"))
            .map(|s| s.to_lowercase())
            .filter(|d| d == "inbound" || d == "outbound"); // descarta "leg_b" u otros valores no-direccionales

        let direction = native_direction.unwrap_or_else(|| {
            if context == "from-pbx" || context.starts_with("from-pbx") {
                "outbound".to_string()
            } else if context == "from-kamailio" || context.starts_with("from-kamailio") {
                "inbound".to_string()
            } else {
                // Tráfico predominante de esta plataforma es entrante (carrier -> extensión)
                "inbound".to_string()
            }
        });

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
        let metrics = LegMetrics::of(event, true);
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

        if let Ok(client) = self.db_pool.get().await {
            match client
                .execute(
                    "UPDATE cdrs SET pdd_ms = LEAST(pdd_ms, $2), mos = LEAST(mos, $3),
                                     rtp_loss_pct = GREATEST(rtp_loss_pct, $4)
                     WHERE call_uuid = $1 AND start_time > now() - INTERVAL '1 day'",
                    &[&a_uuid, &metrics.pdd_ms, &metrics.mos, &metrics.loss_pct],
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
        pending
            .entry(a_uuid)
            .and_modify(|(m, _)| *m = m.merge(metrics))
            .or_insert((metrics, Instant::now()));
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

        // 🔍 Detectar dirección real
        //
        // Fix 2026-08-09 (soporte, revisión completa): mismo criterio que
        // CHANNEL_CREATE - los headers nativos de FreeSWITCH (Call-Direction /
        // Caller-Direction / Caller-Logical-Direction / variable_direction) son
        // la señal confiable, "context" solo describe qué perfil está
        // *entregando* la llamada, no quién la originó. hangup_cause=
        // USER_NOT_REGISTERED se mantiene como red de seguridad adicional
        // (solo le puede pasar al destino, nunca a quien origina), por si
        // algún evento raro llegara sin ninguno de los headers nativos.
        let context = event.get_header("Caller-Context")
            .or_else(|| event.get_header("variable_context"))
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        let native_direction = event.get_header("Call-Direction")
            .or_else(|| event.get_header("Caller-Direction"))
            .or_else(|| event.get_header("Caller-Logical-Direction"))
            .or_else(|| event.get_header("variable_direction"))
            .map(|s| s.to_lowercase())
            .filter(|d| d == "inbound" || d == "outbound");

        let direction = if hangup_cause == "USER_NOT_REGISTERED" {
            "inbound".to_string()
        } else {
            native_direction.unwrap_or_else(|| {
                if context == "from-pbx" || context.starts_with("from-pbx") {
                    "outbound".to_string()
                } else if context == "from-kamailio" || context.starts_with("from-kamailio") {
                    "inbound".to_string()
                } else {
                    "inbound".to_string()
                }
            })
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
            pdd_ms: legs.pdd_ms,
            mos: legs.mos,
            rtp_loss_pct: legs.loss_pct,
        };

        if let Err(e) = self.cdr_generator.generate_cdr(hangup_event).await {
            error!("Error generating CDR for {}: {}", uuid, e);
        }
    }
}