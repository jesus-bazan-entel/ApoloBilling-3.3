//! Plan DTOs
//!
//! Request and response types for plan management endpoints. Every plan is
//! a minute-bucket template (Plan Control / Plan Abierto) — there is no
//! other billing model.

use apolo_core::models::Plan;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use validator::Validate;

fn default_proration_mode() -> String {
    "full".to_string()
}

/// Plan creation request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct PlanCreateRequest {
    /// Plan name
    #[validate(length(min = 1, max = 100, message = "Plan name is required"))]
    pub plan_name: String,

    /// Unique plan code
    #[validate(length(min = 1, max = 50, message = "Plan code is required"))]
    pub plan_code: String,

    /// "control" (blocks new calls once the bucket is exhausted) or
    /// "abierto" (never blocks; overage is tariffed and reported)
    pub bucket_type: String,

    /// Minutes included in the monthly base bucket
    #[validate(range(min = 0, max = 1_000_000, message = "included_minutes out of range"))]
    pub included_minutes: i32,

    /// "full" | "prorated" — how the first cycle is sized when an account
    /// is created mid-month
    #[serde(default = "default_proration_mode")]
    pub proration_mode: String,

    /// Flat overage rate per minute (takes priority over overage_tariff_group)
    pub overage_flat_rate_per_minute: Option<Decimal>,

    /// `rate_cards.card_group` to use for LPM overage tariffing
    pub overage_tariff_group: Option<String>,

    /// Max concurrent calls
    #[serde(default = "default_max_concurrent_calls")]
    #[validate(range(min = 1, max = 100))]
    pub max_concurrent_calls: i32,

    /// Optional description
    pub description: Option<String>,

    /// Whether plan is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_max_concurrent_calls() -> i32 {
    5
}

fn default_enabled() -> bool {
    true
}

impl PlanCreateRequest {
    /// Validate plan business rules
    pub fn validate_business_rules(&self) -> Result<(), String> {
        if self.bucket_type != "control" && self.bucket_type != "abierto" {
            return Err("bucket_type must be 'control' or 'abierto'".to_string());
        }

        if self.proration_mode != "full" && self.proration_mode != "prorated" {
            return Err("proration_mode must be 'full' or 'prorated'".to_string());
        }

        Ok(())
    }
}

/// Plan update request — general template fields only; bucket_type,
/// included_minutes, proration_mode and overage settings are updated via
/// `PUT /plans/{id}/bucket-config`.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct PlanUpdateRequest {
    /// Plan name
    #[validate(length(min = 1, max = 100))]
    pub plan_name: Option<String>,

    /// Max concurrent calls
    #[validate(range(min = 1, max = 100))]
    pub max_concurrent_calls: Option<i32>,

    /// Description
    pub description: Option<String>,

    /// Enabled status
    pub enabled: Option<bool>,
}

/// Plan response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanResponse {
    pub id: i32,
    pub plan_name: String,
    pub plan_code: String,
    pub bucket_type: String,
    pub included_minutes: i32,
    pub proration_mode: String,
    pub overage_flat_rate_per_minute: Option<Decimal>,
    pub overage_tariff_group: Option<String>,
    pub max_concurrent_calls: i32,
    pub description: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
}

impl From<Plan> for PlanResponse {
    fn from(plan: Plan) -> Self {
        Self {
            id: plan.id,
            plan_name: plan.plan_name,
            plan_code: plan.plan_code,
            bucket_type: plan.bucket_type,
            included_minutes: plan.included_minutes,
            proration_mode: plan.proration_mode,
            overage_flat_rate_per_minute: plan.overage_flat_rate_per_minute,
            overage_tariff_group: plan.overage_tariff_group,
            max_concurrent_calls: plan.max_concurrent_calls,
            description: plan.description,
            enabled: plan.enabled,
            created_at: plan.created_at,
            updated_at: plan.updated_at,
            created_by: plan.created_by,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_req() -> PlanCreateRequest {
        PlanCreateRequest {
            plan_name: "Test Plan".to_string(),
            plan_code: "TEST-01".to_string(),
            bucket_type: "control".to_string(),
            included_minutes: 1000,
            proration_mode: "full".to_string(),
            overage_flat_rate_per_minute: None,
            overage_tariff_group: None,
            max_concurrent_calls: 5,
            description: Some("Test".to_string()),
            enabled: true,
        }
    }

    #[test]
    fn test_control_plan_validation() {
        assert!(base_req().validate_business_rules().is_ok());
    }

    #[test]
    fn test_abierto_plan_validation() {
        let mut req = base_req();
        req.bucket_type = "abierto".to_string();
        assert!(req.validate_business_rules().is_ok());
    }

    #[test]
    fn test_invalid_bucket_type_rejected() {
        let mut req = base_req();
        req.bucket_type = "postpaid".to_string();
        assert!(req.validate_business_rules().is_err());
    }

    #[test]
    fn test_invalid_proration_mode_rejected() {
        let mut req = base_req();
        req.proration_mode = "yearly".to_string();
        assert!(req.validate_business_rules().is_err());
    }
}
