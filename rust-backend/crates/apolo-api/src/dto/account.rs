//! Account DTOs
//!
//! Request and response types for account management endpoints.

use apolo_core::models::{Account, AccountStatus};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Account creation request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct AccountCreateRequest {
    /// Unique account number
    #[validate(length(min = 1, max = 50, message = "Account number is required"))]
    pub account_number: String,

    /// Customer phone number for ANI matching
    pub customer_phone: Option<String>,

    /// Maximum concurrent calls
    #[serde(default = "default_max_concurrent_calls")]
    #[validate(range(min = 1, max = 100))]
    pub max_concurrent_calls: i32,

    /// Plan ID — when set, bucket configuration (bucket_type,
    /// included_minutes, proration_mode, overage settings) is copied from
    /// the plan and any bucket fields below are ignored.
    pub plan_id: Option<i32>,

    /// "control" | "abierto" — required when plan_id is not set
    pub bucket_type: Option<String>,

    /// Minutes included in the monthly base bucket — required when plan_id
    /// is not set
    pub included_minutes: Option<i32>,

    /// "full" | "prorated"
    #[serde(default = "default_proration_mode")]
    pub proration_mode: String,

    /// Flat overage rate per minute (takes priority over overage_tariff_group)
    pub overage_flat_rate_per_minute: Option<Decimal>,

    /// `rate_cards.card_group` to use for LPM overage tariffing
    pub overage_tariff_group: Option<String>,

    /// Contact email for 80%/100% consumption alerts
    pub notification_email: Option<String>,
}

fn default_max_concurrent_calls() -> i32 {
    1
}

fn default_proration_mode() -> String {
    "full".to_string()
}

/// Account update request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct AccountUpdateRequest {
    /// Customer phone number
    pub customer_phone: Option<String>,

    /// Account status
    pub status: Option<String>,

    /// Maximum concurrent calls
    #[validate(range(min = 1, max = 100))]
    pub max_concurrent_calls: Option<i32>,

    /// Contact email for 80%/100% consumption alerts
    pub notification_email: Option<String>,
}

/// Account response
#[derive(Debug, Clone, Serialize)]
pub struct AccountResponse {
    /// Account ID
    pub id: i32,

    /// Account number
    pub account_number: String,

    /// Customer phone
    pub customer_phone: Option<String>,

    /// Status
    pub status: String,

    /// Max concurrent calls
    pub max_concurrent_calls: i32,

    /// Plan ID (if account was created from a plan)
    pub plan_id: Option<i32>,

    /// "control" | "abierto"
    pub bucket_type: String,

    /// Minutes included in the monthly base bucket
    pub included_minutes: i32,

    /// "full" | "prorated"
    pub proration_mode: String,

    /// Flat overage rate per minute
    pub overage_flat_rate_per_minute: Option<Decimal>,

    /// `rate_cards.card_group` used for LPM overage tariffing
    pub overage_tariff_group: Option<String>,

    /// Contact email for consumption alerts
    pub notification_email: Option<String>,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

impl From<Account> for AccountResponse {
    fn from(account: Account) -> Self {
        Self {
            id: account.id,
            account_number: account.account_number,
            customer_phone: account.customer_phone,
            status: account.status.to_string(),
            max_concurrent_calls: account.max_concurrent_calls,
            plan_id: account.plan_id,
            bucket_type: account.bucket_type,
            included_minutes: account.included_minutes,
            proration_mode: account.proration_mode,
            overage_flat_rate_per_minute: account.overage_flat_rate_per_minute,
            overage_tariff_group: account.overage_tariff_group,
            notification_email: account.notification_email,
            created_at: account.created_at,
            updated_at: account.updated_at,
        }
    }
}

impl From<&Account> for AccountResponse {
    fn from(account: &Account) -> Self {
        AccountResponse::from(account.clone())
    }
}

/// Account list filter parameters
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AccountFilterParams {
    /// Filter by status
    pub status: Option<String>,

    /// Search by account number or phone
    pub search: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_status_roundtrip() {
        assert_eq!(
            AccountStatus::from_str("active"),
            Some(AccountStatus::Active)
        );
        assert_eq!(AccountStatus::Active.to_string(), "active");
    }
}
