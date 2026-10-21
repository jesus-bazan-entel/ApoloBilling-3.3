// src/models/plan.rs
//! Plan model for account creation templates
//!
//! Plans define preset minute-bucket configurations (Plan Control / Plan
//! Abierto) for account creation.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Plan model representing account creation templates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    /// Unique plan ID
    pub id: i32,

    /// Human-readable plan name (e.g., "Plan 1000 Minutos")
    pub plan_name: String,

    /// Unique plan code (e.g., "P1000M")
    pub plan_code: String,

    /// "control" (blocks new calls once the bucket is exhausted) or
    /// "abierto" (never blocks; overage is tariffed and reported)
    pub bucket_type: String,

    /// Minutes included in the monthly base bucket
    pub included_minutes: i32,

    /// "full" | "prorated" — how the first cycle is sized when an account
    /// is created mid-month
    pub proration_mode: String,

    /// Flat overage rate per minute (takes priority over overage_tariff_group)
    pub overage_flat_rate_per_minute: Option<Decimal>,

    /// `rate_cards.card_group` to use for LPM overage tariffing
    pub overage_tariff_group: Option<String>,

    /// Maximum concurrent calls allowed
    pub max_concurrent_calls: i32,

    /// Optional description
    pub description: Option<String>,

    /// Whether this plan is active/selectable
    pub enabled: bool,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Creator identifier
    pub created_by: String,
}

impl Plan {
    /// Check if plan is valid for use
    pub fn is_active(&self) -> bool {
        self.enabled
    }

    /// Check if this is a Plan Control (blocks on bucket exhaustion)
    pub fn is_control(&self) -> bool {
        self.bucket_type == "control"
    }

    /// Validate plan configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.plan_name.trim().is_empty() {
            return Err("Plan name cannot be empty".to_string());
        }

        if self.plan_code.trim().is_empty() {
            return Err("Plan code cannot be empty".to_string());
        }

        if self.bucket_type != "control" && self.bucket_type != "abierto" {
            return Err("bucket_type must be 'control' or 'abierto'".to_string());
        }

        if self.included_minutes < 0 {
            return Err("Included minutes cannot be negative".to_string());
        }

        if self.max_concurrent_calls <= 0 {
            return Err("Max concurrent calls must be greater than zero".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_plan(bucket_type: &str) -> Plan {
        Plan {
            id: 1,
            plan_name: "Test Plan".to_string(),
            plan_code: "TEST-01".to_string(),
            bucket_type: bucket_type.to_string(),
            included_minutes: 1000,
            proration_mode: "full".to_string(),
            overage_flat_rate_per_minute: None,
            overage_tariff_group: None,
            max_concurrent_calls: 5,
            description: Some("Test description".to_string()),
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: "system".to_string(),
        }
    }

    #[test]
    fn test_control_plan_validation() {
        let plan = create_test_plan("control");
        assert!(plan.validate().is_ok());
        assert!(plan.is_control());
    }

    #[test]
    fn test_abierto_plan_validation() {
        let plan = create_test_plan("abierto");
        assert!(plan.validate().is_ok());
        assert!(!plan.is_control());
    }

    #[test]
    fn test_invalid_bucket_type_rejected() {
        let mut plan = create_test_plan("control");
        plan.bucket_type = "postpaid".to_string();
        assert!(plan.validate().is_err());
    }

    #[test]
    fn test_zero_concurrent_calls_rejected() {
        let mut plan = create_test_plan("control");
        plan.max_concurrent_calls = 0;
        assert!(plan.validate().is_err());
    }
}
