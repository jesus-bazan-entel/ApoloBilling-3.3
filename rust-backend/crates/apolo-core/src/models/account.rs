//! Account model
//!
//! Represents customer accounts in the billing system. Billing behavior
//! (Plan Control / Plan Abierto minute-bucket) is configured separately via
//! the bucket-config endpoints, not modeled on this struct.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

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
/// Represents a customer account in the billing system. Every account bills
/// via the minute-bucket model (Plan Control / Plan Abierto); that
/// configuration lives in its own table, addressed via `/accounts/{id}/bucket-config`.
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

    /// Account status
    pub status: AccountStatus,

    /// Maximum concurrent calls allowed
    pub max_concurrent_calls: i32,

    /// Associated plan (if account was created from a plan)
    pub plan_id: Option<i32>,

    /// "control" (blocks new calls once the bucket is exhausted) or
    /// "abierto" (never blocks; overage is tariffed and reported)
    pub bucket_type: String,

    /// Minutes included in the monthly base bucket
    pub included_minutes: i32,

    /// "full" | "prorated" — how the first cycle is sized when the account
    /// is created mid-month
    pub proration_mode: String,

    /// Flat overage rate per minute (takes priority over overage_tariff_group)
    pub overage_flat_rate_per_minute: Option<Decimal>,

    /// `rate_cards.card_group` to use for LPM overage tariffing
    pub overage_tariff_group: Option<String>,

    /// Contact email for 80%/100% consumption alerts
    pub notification_email: Option<String>,

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
            status: AccountStatus::Active,
            max_concurrent_calls: 1,
            plan_id: None,
            bucket_type: "control".to_string(),
            included_minutes: 0,
            proration_mode: "full".to_string(),
            overage_flat_rate_per_minute: None,
            overage_tariff_group: None,
            notification_email: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suspended_account_cannot_make_calls() {
        let account = Account {
            status: AccountStatus::Suspended,
            ..Default::default()
        };

        assert!(!account.is_active());
    }

    #[test]
    fn test_normalize_phone() {
        assert_eq!(Account::normalize_phone("+1-555-123-4567"), "15551234567");
        assert_eq!(Account::normalize_phone("(555) 123-4567"), "5551234567");
        assert_eq!(Account::normalize_phone("51999888777"), "51999888777");
    }
}
