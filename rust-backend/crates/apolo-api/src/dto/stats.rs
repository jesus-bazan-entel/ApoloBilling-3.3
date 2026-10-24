//! Statistics DTOs
//!
//! Data transfer objects for dashboard statistics endpoints.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Hourly call statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourlyCallStats {
    /// Hour (0-23)
    pub hour: i32,

    /// Hour label (e.g., "0:00", "13:00")
    pub hour_label: String,

    /// Number of calls in this hour
    pub call_count: i64,

    /// Total duration in seconds
    pub total_duration: i64,

    /// Total revenue
    pub total_revenue: Decimal,
}

/// Daily revenue statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyRevenueStats {
    /// Date
    pub date: String,

    /// Day of week (0-6, where 0 is Sunday)
    pub day_of_week: i32,

    /// Day label (e.g., "Dom", "Lun")
    pub day_label: String,

    /// Number of calls
    pub call_count: i64,

    /// Total revenue
    pub revenue: Decimal,

    /// Total minutes
    pub total_minutes: Decimal,
}

/// Balance trend point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceTrendPoint {
    /// Date
    pub date: String,

    /// Day number (1-30)
    pub day: i32,

    /// Total balance across all active accounts
    pub total_balance: Decimal,

    /// Number of active accounts
    pub active_accounts: i64,

    /// Average balance per account
    pub average_balance: Decimal,
}

/// Calls by hour response
#[derive(Debug, Clone, Serialize)]
pub struct CallsByHourResponse {
    /// Hourly statistics for last 24 hours
    pub data: Vec<HourlyCallStats>,
}

/// Revenue by day response
#[derive(Debug, Clone, Serialize)]
pub struct RevenueByDayResponse {
    /// Daily statistics for last 7 days
    pub data: Vec<DailyRevenueStats>,
}

/// Balance trend response
#[derive(Debug, Clone, Serialize)]
pub struct BalanceTrendResponse {
    /// Daily balance trend for last 30 days
    pub data: Vec<BalanceTrendPoint>,
}

/// Call type statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallTypeStats {
    /// Call direction/type (outbound, inbound, internal)
    pub call_type: String,

    /// Display label
    pub label: String,

    /// Number of calls
    pub call_count: i64,

    /// Total duration in seconds
    pub total_duration: i64,

    /// Total cost
    pub total_cost: Decimal,

    /// Percentage of total calls
    pub percentage: f64,
}

/// Zone statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneStats {
    /// Zone ID
    pub zone_id: Option<i32>,

    /// Zone name
    pub zone_name: String,

    /// Number of calls to this zone
    pub call_count: i64,

    /// Total duration in seconds
    pub total_duration: i64,

    /// Total cost
    pub total_cost: Decimal,

    /// Percentage of total calls
    pub percentage: f64,
}

/// Calls by type response
#[derive(Debug, Clone, Serialize)]
pub struct CallsByTypeResponse {
    /// Call type statistics
    pub data: Vec<CallTypeStats>,
}

/// Calls by zone response
#[derive(Debug, Clone, Serialize)]
pub struct CallsByZoneResponse {
    /// Zone statistics
    pub data: Vec<ZoneStats>,
}

/// Traffic statistics by direction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficStats {
    /// Call direction (inbound, outbound)
    pub direction: String,

    /// Display label
    pub label: String,

    /// Total calls
    pub total_calls: i64,

    /// Total minutes
    pub total_minutes: Decimal,

    /// Total revenue
    pub total_revenue: Decimal,

    /// Average call duration in seconds
    pub avg_duration: i32,
}

/// Traffic by direction response
#[derive(Debug, Clone, Serialize)]
pub struct TrafficByDirectionResponse {
    /// Inbound traffic stats
    pub inbound: TrafficStats,

    /// Outbound traffic stats
    pub outbound: TrafficStats,
}

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
    /// Billed amount
    pub revenue: Decimal,
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
}

/// Query string of GET /api/v1/stats/noc
#[derive(Debug, Clone, Deserialize)]
pub struct NocOverviewQuery {
    pub compare: Option<String>,
}
