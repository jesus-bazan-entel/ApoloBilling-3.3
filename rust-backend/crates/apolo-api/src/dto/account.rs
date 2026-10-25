//! Customer (account) DTOs
//!
//! Request and response types for the customer endpoints. This node is an
//! SBC: an account is the subscriber a SIP device belongs to, and its status
//! gates REGISTER. The billing columns still exist in the table (unused) and
//! are left to their database defaults.

use apolo_core::models::{Account, AccountStatus, AccountType};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Customer creation request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct AccountCreateRequest {
    /// Unique account number
    #[validate(length(min = 1, max = 50, message = "Account number is required"))]
    pub account_number: String,

    /// Customer phone number
    pub customer_phone: Option<String>,
}

impl AccountCreateRequest {
    /// Convert to Account entity (billing fields keep neutral values)
    pub fn to_account(&self) -> Account {
        Account {
            id: 0,
            account_number: self.account_number.clone(),
            account_name: None,
            customer_phone: self.customer_phone.clone(),
            account_type: AccountType::Postpaid,
            balance: Decimal::ZERO,
            credit_limit: Decimal::ZERO,
            currency: "PEN".to_string(),
            status: AccountStatus::Active,
            max_concurrent_calls: 5,
            plan_id: None,
            cod_ped: None,
            departamento: None,
            provincia: None,
            distrito: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

/// Customer update request
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct AccountUpdateRequest {
    /// Customer phone number
    pub customer_phone: Option<String>,

    /// Status (active / suspended / closed); anything but active blocks REGISTER
    pub status: Option<String>,
}

/// Customer response
#[derive(Debug, Clone, Serialize)]
pub struct AccountResponse {
    /// Account ID
    pub id: i32,

    /// Account number
    pub account_number: String,

    /// Customer name (CLIENTE)
    pub account_name: Option<String>,

    /// Customer phone
    pub customer_phone: Option<String>,

    /// Status
    pub status: String,

    /// Order code (COD_PED)
    pub cod_ped: Option<String>,

    /// Customer location: department/region
    pub departamento: Option<String>,

    /// Customer location: province
    pub provincia: Option<String>,

    /// Customer location: district
    pub distrito: Option<String>,

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
            account_name: account.account_name,
            customer_phone: account.customer_phone,
            status: account.status.to_string(),
            cod_ped: account.cod_ped,
            departamento: account.departamento,
            provincia: account.provincia,
            distrito: account.distrito,
            created_at: account.created_at,
            updated_at: account.updated_at,
        }
    }
}

impl From<&Account> for AccountResponse {
    fn from(account: &Account) -> Self {
        Self::from(account.clone())
    }
}

/// Customer list filter parameters
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AccountFilterParams {
    /// Filter by status
    pub status: Option<String>,

    /// Search by account number, name or phone
    pub search: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_create_request_to_account() {
        let req = AccountCreateRequest {
            account_number: "ACC001".to_string(),
            customer_phone: Some("+51999888777".to_string()),
        };

        let account = req.to_account();
        assert_eq!(account.account_number, "ACC001");
        assert_eq!(account.status, AccountStatus::Active);
    }

    #[test]
    fn test_account_response_from_account() {
        let account = AccountCreateRequest {
            account_number: "ACC001".to_string(),
            customer_phone: None,
        }
        .to_account();

        let response = AccountResponse::from(&account);
        assert_eq!(response.account_number, "ACC001");
        assert_eq!(response.status, AccountStatus::Active.to_string());
    }
}
