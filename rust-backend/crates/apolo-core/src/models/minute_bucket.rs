//! Minute bucket billing model
//!
//! Every account and plan bills via the "bolsa de minutos" model (Plan
//! Control / Plan Abierto) — there is no other billing model.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Minute bucket behavior when the bucket is exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BucketType {
    /// Blocks authorization of NEW outbound calls once the bucket is
    /// exhausted. A call already in progress when the bucket hits zero is
    /// never cut — its tail minutes bill as overage.
    Control,
    /// Never blocks. Minutes beyond the bucket bill as overage.
    Abierto,
}

impl fmt::Display for BucketType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Control => write!(f, "control"),
            Self::Abierto => write!(f, "abierto"),
        }
    }
}

impl BucketType {
    /// Parse from string (case-insensitive).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "control" => Some(Self::Control),
            "abierto" => Some(Self::Abierto),
            _ => None,
        }
    }
}

/// How the first (partial) cycle's bucket is sized on account creation or
/// plan change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProrationMode {
    /// Full bucket from day 1, regardless of activation date.
    #[default]
    Full,
    /// Bucket sized proportionally to the days remaining in the calendar
    /// month. The computed value is a suggestion the operator can still
    /// edit before saving.
    Prorated,
}

impl fmt::Display for ProrationMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => write!(f, "full"),
            Self::Prorated => write!(f, "prorated"),
        }
    }
}

impl ProrationMode {
    /// Parse from string (case-insensitive). Defaults to `Full`.
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "prorated" => Self::Prorated,
            _ => Self::Full,
        }
    }
}

/// Minute-bucket configuration fields shared by `plans` and `accounts`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketConfig {
    /// control | abierto
    pub bucket_type: BucketType,
    /// Minutes included in the monthly base bucket
    pub included_minutes: i32,
    /// full | prorated
    pub proration_mode: ProrationMode,
    /// Flat overage rate per minute; if set, takes priority over `overage_tariff_group`
    pub overage_flat_rate_per_minute: Option<Decimal>,
    /// `rate_cards.card_group` to use for LPM overage tariffing when no flat rate is set
    pub overage_tariff_group: Option<String>,
}

/// A plan's minute-bucket template (`GET/PUT /plans/{id}/bucket-config`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanBucketConfig {
    /// Plan id
    pub plan_id: i32,
    /// Shared bucket configuration fields
    #[serde(flatten)]
    pub config: BucketConfig,
}

/// An account's effective minute-bucket configuration
/// (`GET/PUT /accounts/{id}/minute-bucket`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBucketConfig {
    /// Account id
    pub account_id: i32,
    /// Shared bucket configuration fields
    #[serde(flatten)]
    pub config: BucketConfig,
    /// Contact email for 80%/100% consumption alerts. `None` = no alerts sent.
    pub notification_email: Option<String>,
}

/// A monthly bucket cycle (one row per account per calendar month it was used).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinuteBucketCycle {
    /// Cycle id
    pub id: i32,
    /// Owning account
    pub account_id: i32,
    /// First day of the calendar month
    pub cycle_start: NaiveDate,
    /// Last day of the calendar month
    pub cycle_end: NaiveDate,
    /// Effective included minutes for this cycle (already prorated/edited if applicable)
    pub included_minutes: i32,
    /// Whether `included_minutes` was computed via proration
    pub is_prorated: bool,
    /// Days considered for proration (traceability)
    pub proration_days: Option<i32>,
    /// Sum of additional minutes purchased within the cycle
    pub additional_minutes: i32,
    /// Minutes held by calls currently in progress (soft-hold)
    pub reserved_minutes: Decimal,
    /// Minutes consumed in-bucket by closed CDRs
    pub consumed_minutes: Decimal,
    /// Minutes billed as overage
    pub overage_minutes: Decimal,
    /// Cost tariffed for overage minutes
    pub overage_cost: Decimal,
    /// When the 80% alert was sent (if ever) — prevents duplicate sends
    pub alert_80_sent_at: Option<DateTime<Utc>>,
    /// When the 100% alert was sent (if ever) — prevents duplicate sends
    pub alert_100_sent_at: Option<DateTime<Utc>>,
    /// open | closed
    pub status: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl MinuteBucketCycle {
    /// Total minutes available in the cycle (base + additions).
    #[inline]
    pub fn total_available(&self) -> i32 {
        self.included_minutes + self.additional_minutes
    }

    /// Minutes still available for new calls (accounts for calls in progress).
    #[inline]
    pub fn available_minutes(&self) -> Decimal {
        Decimal::from(self.total_available()) - self.consumed_minutes - self.reserved_minutes
    }

    /// Utilization percentage (0-100+, can exceed 100 in Abierto).
    pub fn utilization_percent(&self) -> Decimal {
        let total = self.total_available();
        if total <= 0 {
            return Decimal::ZERO;
        }
        (self.consumed_minutes / Decimal::from(total)) * Decimal::from(100)
    }
}

/// A manual or automatic addition of minutes within a cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinuteBucketAddition {
    /// Addition id
    pub id: i32,
    /// Owning account
    pub account_id: i32,
    /// Cycle this addition applies to
    pub cycle_id: i32,
    /// Minutes added
    pub minutes_added: i32,
    /// Optional free-text reason
    pub reason: Option<String>,
    /// Username/operator who registered the addition
    pub created_by: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

/// A pending/sent/failed 80%-or-100% consumption alert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinuteBucketAlert {
    /// Alert id
    pub id: i32,
    /// Owning account
    pub account_id: i32,
    /// Cycle that triggered the alert
    pub cycle_id: i32,
    /// 80 or 100
    pub threshold_pct: i16,
    /// pending | sent | failed
    pub status: String,
    /// Error detail if status = failed
    pub error_message: Option<String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// When the email was actually sent
    pub sent_at: Option<DateTime<Utc>>,
}

/// One row of the monthly minute-bucket consumption report
/// (`GET /reports/minute-bucket`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinuteBucketReportRow {
    /// Account id
    pub account_id: i32,
    /// Account number (external identifier)
    pub account_number: String,
    /// Account display name
    pub account_name: Option<String>,
    /// control | abierto
    pub bucket_type: String,
    /// Base minutes included for the cycle
    pub included_minutes: i32,
    /// Whether the base was prorated
    pub is_prorated: bool,
    /// Total minutes added within the cycle
    pub additional_minutes: i32,
    /// Number of individual top-ups within the cycle
    pub additions_count: i64,
    /// Minutes consumed in-bucket
    pub consumed_minutes: Decimal,
    /// Minutes billed as overage
    pub overage_minutes: Decimal,
    /// Cost tariffed for overage
    pub overage_cost: Decimal,
    /// Utilization percentage of (included + additional)
    pub utilization_percent: Decimal,
    /// open | closed
    pub cycle_status: String,
    /// True only for bucket_type=control with no minutes left
    pub is_blocked: bool,
}
