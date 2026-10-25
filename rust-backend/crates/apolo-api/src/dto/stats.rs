//! Statistics DTOs
//!
//! Data transfer objects for the NOC dashboard overview.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ============================================================================
// NOC overview (carrier-class dashboard)
// ============================================================================

/// Call counters for one time window. Ratios (ASR, ACD, NER) are derived by
/// the client from these raw counts so every view computes them the same way.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NocTotals {
    /// Call attempts (every CDR)
    pub attempts: i64,
    /// Calls with an answer_time
    pub answered: i64,
    /// Billed seconds of answered calls
    pub billsec: i64,
    /// Unanswered calls whose hangup cause is a network failure (for NER)
    pub network_failures: i64,
}

/// Headline counters: today so far, the comparison day up to the same time of
/// day (the fair comparison), and the whole comparison day.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NocKpis {
    pub today: NocTotals,
    pub compare_same_time: NocTotals,
    pub compare_full_day: NocTotals,
}

/// Values of one 15-minute slot
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NocSlotValues {
    pub attempts: i64,
    pub answered: i64,
    /// Calls in progress at the middle of the slot
    pub concurrent: i64,
}

/// One 15-minute slot of the day (local time)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NocSlot {
    /// Slot index 0..95
    pub slot: i32,
    /// "HH:MM" local start time
    pub label: String,
    /// None for slots that have not finished yet today
    pub today: Option<NocSlotValues>,
    pub compare: NocSlotValues,
}

/// Unanswered calls by hangup cause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NocCause {
    pub cause: String,
    /// "network" (counts against NER), "user", "cancel" or "other"
    pub category: String,
    pub today: i64,
    /// Comparison day up to the same time of day
    pub compare: i64,
}

/// Traffic of one called-number prefix through one path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NocRange {
    /// First 4 digits of the called number
    pub prefix: String,
    /// "freeswitch" or "direct" (Kamailio straight to the trunk)
    pub route: String,
    pub attempts: i64,
    pub answered: i64,
    pub billsec: i64,
    /// Attempts on the comparison day up to the same time of day
    pub compare_attempts: i64,
    /// Attempts per local hour of today (24 values, 0 for future hours)
    pub hourly_attempts: Vec<i64>,
}

/// Operator-defined levels from system_settings (noc_* keys)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NocThresholds {
    /// ASR (%) below this is "Vigilar"
    pub asr_warn: f64,
    /// ASR (%) below this is "Crítico"
    pub asr_crit: f64,
    pub ner_warn: f64,
    pub ner_crit: f64,
    /// Ranges with fewer attempts today are hidden by default
    pub min_range_attempts: i64,
}

/// Channel and call-rate headroom
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NocCapacity {
    /// Licensed / configured simultaneous channels (noc_max_channels), if set
    pub max_channels: Option<i64>,
    /// Configured CPS limit of the SBC / trunk (noc_max_cps), if set
    pub max_cps: Option<f64>,
    /// Highest calls started in one second during the last hour (from CDRs)
    pub peak_cps_last_hour: i64,
    /// Average calls per second during the last hour (from CDRs)
    pub avg_cps_last_hour: f64,
}

/// Media quality as RTPEngine reports it, over a recent window of the
/// monitoring collector's samples, plus PDD from the CDRs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NocQuality {
    /// Start of the window the media figures cover
    pub window_start: Option<DateTime<Utc>>,
    /// Newest sample in the window
    pub as_of: Option<DateTime<Utc>>,
    /// 15 (1-minute samples) or 60 (hourly rollup fallback)
    pub window_minutes: i32,
    /// RTPEngine's running averages (since it started), latest sample
    pub mos: Option<f64>,
    pub jitter_ms: Option<f64>,
    /// Lost / relayed packets within the window, in percent
    pub packet_loss_pct: Option<f64>,
    /// Sessions past ringing with media missing in one or both directions
    pub one_sided_sessions: Option<f64>,
    pub active_sessions: Option<f64>,
    /// Post-Dial Delay of calls started in the last hour (FreeSWITCH path)
    pub pdd_p50_ms: Option<i32>,
    pub pdd_p95_ms: Option<i32>,
    pub pdd_samples: i64,
}

/// Chart annotation (incident, campaign, maintenance, route migration...)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NocEvent {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub created_by: Option<String>,
}

/// POST /api/v1/stats/noc/events
#[derive(Debug, Clone, Deserialize)]
pub struct NocEventRequest {
    pub kind: String,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
}

/// GET /api/v1/stats/noc
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NocOverviewResponse {
    pub timezone: String,
    /// "yesterday" or "week" (same weekday last week)
    pub compare: String,
    pub day_start: DateTime<Utc>,
    pub compare_day_start: DateTime<Utc>,
    pub generated_at: DateTime<Utc>,
    /// Slot in progress now (not yet in `today`)
    pub current_slot: i32,
    pub slot_minutes: i32,
    pub kpis: NocKpis,
    pub series: Vec<NocSlot>,
    pub causes: Vec<NocCause>,
    pub ranges: Vec<NocRange>,
    pub thresholds: NocThresholds,
    pub capacity: NocCapacity,
    pub quality: NocQuality,
    /// Annotations overlapping today or the comparison day
    pub events: Vec<NocEvent>,
}

/// Query string of GET /api/v1/stats/noc
#[derive(Debug, Clone, Deserialize)]
pub struct NocOverviewQuery {
    pub compare: Option<String>,
}
