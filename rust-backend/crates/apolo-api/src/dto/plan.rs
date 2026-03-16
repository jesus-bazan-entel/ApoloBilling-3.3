//! Plan DTOs
//!
//! Request and response types for plan management endpoints.

use apolo_core::models::{AccountType, Plan};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Plan creation request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct PlanCreateRequest {
    /// Plan name
    #[validate(length(min = 1, max = 100, message = "Plan name is required"))]
    pub plan_name: String,

    /// Unique plan code
    #[validate(length(min = 1, max = 50, message = "Plan code is required"))]
    pub plan_code: String,

    /// Account type this plan is for
    #[validate(length(min = 1, message = "Account type is required"))]
    pub account_type: String,

    /// Initial balance to assign
    #[serde(default)]
    pub initial_balance: Decimal,

    /// Credit limit for postpaid
    #[serde(default)]
    pub credit_limit: Decimal,

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
        let account_type = AccountType::from_str(&self.account_type)
            .ok_or_else(|| "Invalid account type".to_string())?;

        match account_type {
            AccountType::Prepaid => {
                // PREPAID: initial_balance MUST be > 0 (no unlimited plans)
                if self.initial_balance <= Decimal::ZERO {
                    return Err("Prepaid plans must have initial_balance > 0".to_string());
                }

                // Prepaid plans cannot have credit limit
                if self.credit_limit > Decimal::ZERO {
                    return Err("Prepaid plans cannot have credit limit".to_string());
                }
            }
            AccountType::Postpaid => {
                // POSTPAID: credit_limit MUST be > 0 (no unlimited plans)
                if self.credit_limit <= Decimal::ZERO {
                    return Err("Postpaid plans must have credit_limit > 0".to_string());
                }

                // Postpaid plans typically start with balance = 0
                if self.initial_balance < Decimal::ZERO {
                    return Err("Initial balance cannot be negative".to_string());
                }
            }
        }

        Ok(())
    }
}

/// Plan update request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct PlanUpdateRequest {
    /// Plan name
    #[validate(length(min = 1, max = 100))]
    pub plan_name: Option<String>,

    /// Initial balance
    pub initial_balance: Option<Decimal>,

    /// Credit limit
    pub credit_limit: Option<Decimal>,

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
    pub account_type: String,
    pub initial_balance: Decimal,
    pub credit_limit: Decimal,
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
            account_type: plan.account_type.to_string(),
            initial_balance: plan.initial_balance,
            credit_limit: plan.credit_limit,
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
    use rust_decimal_macros::dec;

    #[test]
    fn test_prepaid_plan_validation() {
        let req = PlanCreateRequest {
            plan_name: "Test Prepaid".to_string(),
            plan_code: "TEST-PRE".to_string(),
            account_type: "PREPAID".to_string(),
            initial_balance: dec!(50.00),
            credit_limit: dec!(0.00),
            max_concurrent_calls: 5,
            description: Some("Test".to_string()),
            enabled: true,
        };

        assert!(req.validate_business_rules().is_ok());
    }

    #[test]
    fn test_prepaid_cannot_have_credit_limit() {
        let req = PlanCreateRequest {
            plan_name: "Test Prepaid".to_string(),
            plan_code: "TEST-PRE".to_string(),
            account_type: "PREPAID".to_string(),
            initial_balance: dec!(0.00),
            credit_limit: dec!(100.00), // Invalid
            max_concurrent_calls: 5,
            description: None,
            enabled: true,
        };

        assert!(req.validate_business_rules().is_err());
    }

    #[test]
    fn test_postpaid_plan_validation() {
        let req = PlanCreateRequest {
            plan_name: "Test Postpaid".to_string(),
            plan_code: "TEST-POST".to_string(),
            account_type: "POSTPAID".to_string(),
            initial_balance: dec!(0.00),
            credit_limit: dec!(500.00),
            max_concurrent_calls: 10,
            description: Some("Test".to_string()),
            enabled: true,
        };

        assert!(req.validate_business_rules().is_ok());
    }

    #[test]
    fn test_negative_balance_rejected() {
        let req = PlanCreateRequest {
            plan_name: "Test".to_string(),
            plan_code: "TEST".to_string(),
            account_type: "PREPAID".to_string(),
            initial_balance: dec!(-10.00), // Invalid
            credit_limit: dec!(0.00),
            max_concurrent_calls: 5,
            description: None,
            enabled: true,
        };

        assert!(req.validate_business_rules().is_err());
    }
}
