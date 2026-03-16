// src/models/account.rs
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: i32,  // PostgreSQL integer = i32, not i64
    pub account_number: String,
    pub account_type: AccountType,
    pub balance: Decimal,
    pub credit_limit: Decimal,
    pub currency: String,
    pub status: AccountStatus,
    pub max_concurrent_calls: Option<i32>, 
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    Prepaid,
    Postpaid,
}

impl AccountType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "prepaid" => AccountType::Prepaid,
            "postpaid" => AccountType::Postpaid,
            _ => AccountType::Prepaid,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            AccountType::Prepaid => "prepaid",
            AccountType::Postpaid => "postpaid",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccountStatus {
    Active,
    Suspended,
    Closed,
}

impl AccountStatus {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "active" => AccountStatus::Active,
            "suspended" => AccountStatus::Suspended,
            "closed" => AccountStatus::Closed,
            _ => AccountStatus::Active,
        }
    }
}

impl Account {
    pub fn is_active(&self) -> bool {
        self.status == AccountStatus::Active
    }

    pub fn can_authorize(&self, required_balance: Decimal) -> bool {
        match self.account_type {
            AccountType::Prepaid => self.balance >= required_balance,
            AccountType::Postpaid => {
                let total_debt = self.balance.abs();
                total_debt < self.credit_limit
            }
        }
    }
}
