//! Minute bucket DTOs
//!
//! Request/response types for the "bolsa de minutos" billing model endpoints.
//! Every account and plan uses this model — there is no other.

use apolo_core::models::{BucketConfig, BucketType};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Shared request body for `PUT /plans/{id}/bucket-config` and
/// `PUT /accounts/{id}/bucket-config`.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct BucketConfigRequest {
    /// "control" | "abierto"
    pub bucket_type: String,

    /// Minutes included in the monthly base bucket
    #[validate(range(min = 0, max = 1_000_000, message = "included_minutes out of range"))]
    #[serde(default)]
    pub included_minutes: i32,

    /// "full" | "prorated"
    #[serde(default = "default_proration_mode")]
    pub proration_mode: String,

    /// Flat overage rate per minute (takes priority over overage_tariff_group)
    pub overage_flat_rate_per_minute: Option<Decimal>,

    /// `rate_cards.card_group` to use for LPM overage tariffing
    pub overage_tariff_group: Option<String>,
}

fn default_proration_mode() -> String {
    "full".to_string()
}

impl BucketConfigRequest {
    /// Cross-field validation not expressible with `validator` derive macros.
    pub fn validate_business(&self) -> Result<(), String> {
        if BucketType::from_str(&self.bucket_type).is_none() {
            return Err(format!("Invalid bucket_type: {}", self.bucket_type));
        }

        if !matches!(self.proration_mode.as_str(), "full" | "prorated") {
            return Err(format!("Invalid proration_mode: {}", self.proration_mode));
        }

        Ok(())
    }

    /// Convert to the shared core config type. Callers must run
    /// `validate_business()` first.
    pub fn to_config(&self) -> BucketConfig {
        BucketConfig {
            bucket_type: BucketType::from_str(&self.bucket_type).unwrap_or(BucketType::Control),
            included_minutes: self.included_minutes,
            proration_mode: apolo_core::models::ProrationMode::from_str(&self.proration_mode),
            overage_flat_rate_per_minute: self.overage_flat_rate_per_minute,
            overage_tariff_group: self.overage_tariff_group.clone(),
        }
    }
}

/// Request body for `PUT /accounts/{id}/bucket-config` — bucket config plus
/// the notification email used for 80%/100% consumption alerts.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct AccountBucketConfigRequest {
    /// Shared bucket configuration fields
    #[serde(flatten)]
    pub config: BucketConfigRequest,

    /// Contact email for consumption alerts. `None`/omitted leaves the
    /// current value unchanged; pass an empty string to clear it.
    pub notification_email: Option<String>,
}

/// Request body for `POST /accounts/{id}/minute-bucket/topup`.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct MinuteBucketTopupRequest {
    /// Minutes to add to the current open cycle
    #[validate(range(min = 1, max = 100_000, message = "minutes_added out of range"))]
    pub minutes_added: i32,

    /// Optional free-text reason (order id, operator note, etc.)
    #[validate(length(max = 255))]
    pub reason: Option<String>,
}

/// Request body for `PUT /accounts/{id}/minute-bucket` — manual override of
/// the currently open cycle's `included_minutes` (e.g. adjusting a prorated
/// suggestion by hand).
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct MinuteBucketCycleEditRequest {
    /// New effective included_minutes for the open cycle
    #[validate(range(min = 0, max = 1_000_000, message = "included_minutes out of range"))]
    pub included_minutes: i32,

    /// Reason for the manual override (required for audit trail)
    #[validate(length(min = 1, max = 255, message = "reason is required"))]
    pub reason: String,
}

/// Live status of an account's current bucket cycle
/// (`GET /accounts/{id}/minute-bucket`).
#[derive(Debug, Clone, Serialize)]
pub struct MinuteBucketStatusResponse {
    /// Account id
    pub account_id: i32,
    /// Account number
    pub account_number: String,
    /// "control" | "abierto"
    pub bucket_type: String,
    /// First day of the current cycle
    pub cycle_start: chrono::NaiveDate,
    /// Last day of the current cycle
    pub cycle_end: chrono::NaiveDate,
    /// Base minutes included this cycle
    pub included_minutes: i32,
    /// Whether the base was prorated
    pub is_prorated: bool,
    /// Minutes added via top-ups this cycle
    pub additional_minutes: i32,
    /// Minutes consumed in-bucket
    pub consumed_minutes: Decimal,
    /// Minutes billed as overage
    pub overage_minutes: Decimal,
    /// Cost tariffed for overage
    pub overage_cost: Decimal,
    /// Minutes still available for new calls
    pub available_minutes: Decimal,
    /// Utilization percentage of (included + additional)
    pub utilization_percent: Decimal,
    /// True only for bucket_type=control with no minutes left
    pub is_blocked: bool,
    /// "flat" | "rate_card_group:<name>" | "standard"
    pub overage_rate_source: String,
    /// "open" | "closed"
    pub status: String,
}

/// Query params for `GET /reports/minute-bucket` (and its `/export` twin).
#[derive(Debug, Clone, Deserialize)]
pub struct MinuteBucketReportQuery {
    /// Target month, "YYYY-MM" or "YYYY-MM-DD" (defaults to current month)
    pub month: Option<String>,
    /// Filter to a single account
    pub account_id: Option<i32>,
    /// Filter by "control" | "abierto"
    pub bucket_type: Option<String>,
}
