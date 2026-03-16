//! Account model
//!
//! Represents customer accounts in the billing system, supporting both
//! prepaid and postpaid billing models.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Account type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    /// Prepaid account - must have positive balance
    #[default]
    Prepaid,
    /// Postpaid account - can use credit up to limit
    Postpaid,
}

impl fmt::Display for AccountType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccountType::Prepaid => write!(f, "PREPAID"),
            AccountType::Postpaid => write!(f, "POSTPAID"),
        }
    }
}

impl AccountType {
    /// Parse from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "prepaid" => Some(AccountType::Prepaid),
            "postpaid" => Some(AccountType::Postpaid),
            _ => None,
        }
    }
}

/// Account status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AccountStatus {
    /// Active account - can make and receive calls
    #[default]
    Active,
    /// Suspended account - temporarily blocked
    Suspended,
    /// Closed account - permanently deactivated
    Closed,
}

impl fmt::Display for AccountStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccountStatus::Active => write!(f, "active"),
            AccountStatus::Suspended => write!(f, "suspended"),
            AccountStatus::Closed => write!(f, "closed"),
        }
    }
}

impl AccountStatus {
    /// Parse from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "active" => Some(AccountStatus::Active),
            "suspended" => Some(AccountStatus::Suspended),
            "closed" => Some(AccountStatus::Closed),
            _ => None,
        }
    }

    /// Check if account can make calls
    pub fn can_make_calls(&self) -> bool {
        matches!(self, AccountStatus::Active)
    }
}

/// Account entity
///
/// Represents a customer account in the billing system.
/// Accounts can be prepaid (require positive balance) or postpaid (use credit).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// Unique identifier
    pub id: i32,

    /// Account number (unique identifier for external systems)
    pub account_number: String,

    /// Customer name
    pub account_name: Option<String>,

    /// Associated phone number for ANI matching
    pub customer_phone: Option<String>,

    /// Account billing type
    pub account_type: AccountType,

    /// Current balance (positive for prepaid, can be negative for postpaid)
    pub balance: Decimal,

    /// Credit limit for postpaid accounts
    pub credit_limit: Decimal,

    /// Currency code (ISO 4217)
    pub currency: String,

    /// Account status
    pub status: AccountStatus,

    /// Maximum concurrent calls allowed
    pub max_concurrent_calls: i32,

    /// Associated plan (if account was created from a plan)
    pub plan_id: Option<i32>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl Account {
    /// Check if account is active
    #[inline]
    pub fn is_active(&self) -> bool {
        self.status.can_make_calls()
    }

    /// Get available balance for authorization
    ///
    /// For prepaid: current balance
    /// For postpaid: balance + credit_limit
    #[inline]
    pub fn available_balance(&self) -> Decimal {
        match self.account_type {
            AccountType::Prepaid => self.balance,
            AccountType::Postpaid => self.balance + self.credit_limit,
        }
    }

    /// Check if account can authorize a specific amount
    pub fn can_authorize(&self, required_amount: Decimal) -> bool {
        self.is_active() && self.available_balance() >= required_amount
    }

    /// Check if account has deficit (negative balance)
    pub fn has_deficit(&self) -> bool {
        self.balance < Decimal::ZERO
    }

    /// Get current deficit amount (0 if no deficit)
    pub fn deficit_amount(&self) -> Decimal {
        if self.balance < Decimal::ZERO {
            self.balance.abs()
        } else {
            Decimal::ZERO
        }
    }

    /// Normalize phone number for matching
    pub fn normalize_phone(phone: &str) -> String {
        phone.chars().filter(|c| c.is_ascii_digit()).collect()
    }
}

impl Default for Account {
    fn default() -> Self {
        Self {
            id: 0,
            account_number: String::new(),
            account_name: None,
            customer_phone: None,
            account_type: AccountType::Prepaid,
            balance: Decimal::ZERO,
            credit_limit: Decimal::ZERO,
            currency: "USD".to_string(),
            status: AccountStatus::Active,
            max_concurrent_calls: 1,
            plan_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_prepaid_available_balance() {
        let account = Account {
            account_type: AccountType::Prepaid,
            balance: dec!(100.00),
            credit_limit: dec!(50.00), // Should be ignored for prepaid
            ..Default::default()
        };

        assert_eq!(account.available_balance(), dec!(100.00));
    }

    #[test]
    fn test_postpaid_available_balance() {
        let account = Account {
            account_type: AccountType::Postpaid,
            balance: dec!(-50.00),
            credit_limit: dec!(100.00),
            ..Default::default()
        };

        assert_eq!(account.available_balance(), dec!(50.00));
    }

    #[test]
    fn test_can_authorize() {
        let account = Account {
            account_type: AccountType::Prepaid,
            balance: dec!(10.00),
            status: AccountStatus::Active,
            ..Default::default()
        };

        assert!(account.can_authorize(dec!(5.00)));
        assert!(account.can_authorize(dec!(10.00)));
        assert!(!account.can_authorize(dec!(10.01)));
    }

    #[test]
    fn test_suspended_account_cannot_authorize() {
        let account = Account {
            balance: dec!(100.00),
            status: AccountStatus::Suspended,
            ..Default::default()
        };

        assert!(!account.can_authorize(dec!(1.00)));
    }

    #[test]
    fn test_normalize_phone() {
        assert_eq!(Account::normalize_phone("+1-555-123-4567"), "15551234567");
        assert_eq!(Account::normalize_phone("(555) 123-4567"), "5551234567");
        assert_eq!(Account::normalize_phone("51999888777"), "51999888777");
    }
}
