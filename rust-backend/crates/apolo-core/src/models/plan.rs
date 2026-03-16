// src/models/plan.rs
//! Plan model for account creation templates
//!
//! Plans define preset configurations for account creation,
//! standardizing initial_balance, credit_limit, and other settings.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::AccountType;

/// Plan model representing account creation templates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    /// Unique plan ID
    pub id: i32,

    /// Human-readable plan name (e.g., "Prepago Básico")
    pub plan_name: String,

    /// Unique plan code (e.g., "PRE-BAS", "POST-500")
    pub plan_code: String,

    /// Account type this plan is for
    pub account_type: AccountType,

    /// Initial balance to assign when creating account
    pub initial_balance: Decimal,

    /// Credit limit for postpaid accounts (0 for prepaid)
    pub credit_limit: Decimal,

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

    /// Check if this is a prepaid plan
    pub fn is_prepaid(&self) -> bool {
        matches!(self.account_type, AccountType::Prepaid)
    }

    /// Check if this is a postpaid plan
    pub fn is_postpaid(&self) -> bool {
        matches!(self.account_type, AccountType::Postpaid)
    }

    /// Validate plan configuration
    pub fn validate(&self) -> Result<(), String> {
        // Plan name must not be empty
        if self.plan_name.trim().is_empty() {
            return Err("Plan name cannot be empty".to_string());
        }

        // Plan code must not be empty
        if self.plan_code.trim().is_empty() {
            return Err("Plan code cannot be empty".to_string());
        }

        // Initial balance must be non-negative
        if self.initial_balance < Decimal::ZERO {
            return Err("Initial balance cannot be negative".to_string());
        }

        // Credit limit must be non-negative
        if self.credit_limit < Decimal::ZERO {
            return Err("Credit limit cannot be negative".to_string());
        }

        // Prepaid plans should have zero credit limit
        if self.is_prepaid() && self.credit_limit > Decimal::ZERO {
            return Err("Prepaid plans cannot have credit limit".to_string());
        }

        // Max concurrent calls must be positive
        if self.max_concurrent_calls <= 0 {
            return Err("Max concurrent calls must be greater than zero".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_plan(account_type: AccountType, credit_limit: Decimal) -> Plan {
        Plan {
            id: 1,
            plan_name: "Test Plan".to_string(),
            plan_code: "TEST-01".to_string(),
            account_type,
            initial_balance: dec!(0.00),
            credit_limit,
            max_concurrent_calls: 5,
            description: Some("Test description".to_string()),
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: "system".to_string(),
        }
    }

    #[test]
    fn test_prepaid_plan_validation() {
        let plan = create_test_plan(AccountType::Prepaid, dec!(0.00));
        assert!(plan.validate().is_ok());
        assert!(plan.is_prepaid());
        assert!(!plan.is_postpaid());
    }

    #[test]
    fn test_prepaid_plan_cannot_have_credit_limit() {
        let plan = create_test_plan(AccountType::Prepaid, dec!(100.00));
        assert!(plan.validate().is_err());
    }

    #[test]
    fn test_postpaid_plan_validation() {
        let plan = create_test_plan(AccountType::Postpaid, dec!(500.00));
        assert!(plan.validate().is_ok());
        assert!(plan.is_postpaid());
        assert!(!plan.is_prepaid());
    }

    #[test]
    fn test_negative_balance_rejected() {
        let mut plan = create_test_plan(AccountType::Prepaid, dec!(0.00));
        plan.initial_balance = dec!(-10.00);
        assert!(plan.validate().is_err());
    }

    #[test]
    fn test_zero_concurrent_calls_rejected() {
        let mut plan = create_test_plan(AccountType::Prepaid, dec!(0.00));
        plan.max_concurrent_calls = 0;
        assert!(plan.validate().is_err());
    }
}
