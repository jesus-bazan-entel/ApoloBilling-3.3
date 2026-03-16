//! Account DTOs
//!
//! Request and response types for account management endpoints.

use apolo_core::models::{Account, AccountStatus, AccountType};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
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

    /// Account type (prepaid/postpaid)
    #[serde(default = "default_account_type")]
    pub account_type: String,

    /// Credit limit for postpaid accounts
    #[serde(default)]
    pub credit_limit: Decimal,

    /// Currency code (ISO 4217)
    #[serde(default = "default_currency")]
    pub currency: String,

    /// Maximum concurrent calls
    #[serde(default = "default_max_concurrent_calls")]
    #[validate(range(min = 1, max = 100))]
    pub max_concurrent_calls: i32,

    /// Initial balance (optional)
    #[serde(default)]
    pub initial_balance: Decimal,

    /// Plan ID (optional - if set, account will be created with plan settings)
    pub plan_id: Option<i32>,
}

fn default_account_type() -> String {
    "prepaid".to_string()
}

fn default_currency() -> String {
    "USD".to_string()
}

fn default_max_concurrent_calls() -> i32 {
    1
}

impl AccountCreateRequest {
    /// Convert to Account entity
    pub fn to_account(&self) -> Account {
        Account {
            id: 0,
            account_number: self.account_number.clone(),
            account_name: None,
            customer_phone: self.customer_phone.clone(),
            account_type: AccountType::from_str(&self.account_type).unwrap_or(AccountType::Prepaid),
            balance: self.initial_balance,
            credit_limit: self.credit_limit,
            currency: self.currency.clone(),
            status: AccountStatus::Active,
            max_concurrent_calls: self.max_concurrent_calls,
            plan_id: self.plan_id,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

/// Account update request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct AccountUpdateRequest {
    /// Customer phone number
    pub customer_phone: Option<String>,

    /// Account status
    pub status: Option<String>,

    /// Credit limit
    pub credit_limit: Option<Decimal>,

    /// Maximum concurrent calls
    #[validate(range(min = 1, max = 100))]
    pub max_concurrent_calls: Option<i32>,
}

/// Balance top-up request
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TopupRequest {
    /// Amount to add to balance (must be positive, validated in handler)
    pub amount: Decimal,

    /// Optional reason for the topup
    pub reason: Option<String>,
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

    /// Account type
    pub account_type: String,

    /// Current balance
    pub balance: Decimal,

    /// Credit limit
    pub credit_limit: Decimal,

    /// Currency
    pub currency: String,

    /// Status
    pub status: String,

    /// Max concurrent calls
    pub max_concurrent_calls: i32,

    /// Available balance (balance + credit_limit for postpaid)
    pub available_balance: Decimal,

    /// Plan ID (if account was created from a plan)
    pub plan_id: Option<i32>,

    /// Consumed credit (only for postpaid accounts with negative balance)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumed_credit: Option<Decimal>,

    /// Credit utilization percentage (only for postpaid accounts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utilization_percent: Option<f64>,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
}

impl From<Account> for AccountResponse {
    fn from(account: Account) -> Self {
        let available = account.available_balance();

        // Calculate consumed_credit and utilization only for postpaid accounts
        let (consumed, utilization) = if account.account_type == AccountType::Postpaid {
            // Consumed = absolute value of negative balance
            let consumed = if account.balance < Decimal::ZERO {
                account.balance.abs()
            } else {
                Decimal::ZERO
            };

            // Utilization = (consumed / credit_limit) * 100
            let util = if account.credit_limit > Decimal::ZERO {
                let percent = (consumed / account.credit_limit) * Decimal::from(100);
                Some(percent.to_f64().unwrap_or(0.0))
            } else {
                None
            };

            (Some(consumed), util)
        } else {
            (None, None)
        };

        Self {
            id: account.id,
            account_number: account.account_number,
            customer_phone: account.customer_phone,
            account_type: account.account_type.to_string(),
            balance: account.balance,
            credit_limit: account.credit_limit,
            currency: account.currency,
            status: account.status.to_string(),
            max_concurrent_calls: account.max_concurrent_calls,
            available_balance: available,
            plan_id: account.plan_id,
            consumed_credit: consumed,
            utilization_percent: utilization,
            created_at: account.created_at,
            updated_at: account.updated_at,
        }
    }
}

impl From<&Account> for AccountResponse {
    fn from(account: &Account) -> Self {
        let available = account.available_balance();

        // Calculate consumed_credit and utilization only for postpaid accounts
        let (consumed, utilization) = if account.account_type == AccountType::Postpaid {
            let consumed = if account.balance < Decimal::ZERO {
                account.balance.abs()
            } else {
                Decimal::ZERO
            };

            let util = if account.credit_limit > Decimal::ZERO {
                let percent = (consumed / account.credit_limit) * Decimal::from(100);
                Some(percent.to_f64().unwrap_or(0.0))
            } else {
                None
            };

            (Some(consumed), util)
        } else {
            (None, None)
        };

        Self {
            id: account.id,
            account_number: account.account_number.clone(),
            customer_phone: account.customer_phone.clone(),
            account_type: account.account_type.to_string(),
            balance: account.balance,
            credit_limit: account.credit_limit,
            currency: account.currency.clone(),
            status: account.status.to_string(),
            max_concurrent_calls: account.max_concurrent_calls,
            available_balance: available,
            plan_id: account.plan_id,
            consumed_credit: consumed,
            utilization_percent: utilization,
            created_at: account.created_at,
            updated_at: account.updated_at,
        }
    }
}

/// Top-up response
#[derive(Debug, Clone, Serialize)]
pub struct TopupResponse {
    /// Status message
    pub status: String,

    /// Previous balance
    pub previous_balance: Decimal,

    /// Amount added
    pub amount: Decimal,

    /// New balance
    pub new_balance: Decimal,
}

impl TopupResponse {
    pub fn new(previous_balance: Decimal, amount: Decimal, new_balance: Decimal) -> Self {
        Self {
            status: "success".to_string(),
            previous_balance,
            amount,
            new_balance,
        }
    }
}

/// Account list filter parameters
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AccountFilterParams {
    /// Filter by status
    pub status: Option<String>,

    /// Filter by account type
    pub account_type: Option<String>,

    /// Search by account number or phone
    pub search: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_account_create_request_to_account() {
        let req = AccountCreateRequest {
            account_number: "ACC001".to_string(),
            customer_phone: Some("+51999888777".to_string()),
            account_type: "prepaid".to_string(),
            credit_limit: dec!(0),
            currency: "USD".to_string(),
            max_concurrent_calls: 2,
            initial_balance: dec!(100.00),
            plan_id: None,
        };

        let account = req.to_account();
        assert_eq!(account.account_number, "ACC001");
        assert_eq!(account.balance, dec!(100.00));
        assert_eq!(account.account_type, AccountType::Prepaid);
        assert_eq!(account.plan_id, None);
    }

    #[test]
    fn test_account_response_from_account() {
        let account = Account {
            id: 1,
            account_number: "ACC001".to_string(),
            account_name: None,
            customer_phone: Some("+51999888777".to_string()),
            account_type: AccountType::Prepaid,
            balance: dec!(100.00),
            credit_limit: dec!(0),
            currency: "USD".to_string(),
            status: AccountStatus::Active,
            max_concurrent_calls: 1,
            plan_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let response = AccountResponse::from(&account);
        assert_eq!(response.id, 1);
        assert_eq!(response.available_balance, dec!(100.00));
        assert_eq!(response.plan_id, None);
        assert_eq!(response.consumed_credit, None); // Prepaid doesn't have consumed_credit
        assert_eq!(response.utilization_percent, None);
    }

    #[test]
    fn test_topup_response() {
        let response = TopupResponse::new(dec!(100.00), dec!(50.00), dec!(150.00));
        assert_eq!(response.status, "success");
        assert_eq!(response.new_balance, dec!(150.00));
    }
}
