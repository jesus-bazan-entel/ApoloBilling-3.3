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
