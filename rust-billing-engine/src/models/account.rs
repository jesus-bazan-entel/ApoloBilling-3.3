// src/models/account.rs
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: i32,  // PostgreSQL integer = i32, not i64
    pub account_number: String,
    pub status: AccountStatus,
    pub max_concurrent_calls: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// "control" | "abierto" — see minute_bucket_manager.rs for the bolsa-de-minutos engine
    pub bucket_type: Option<String>,
    /// Flat overage rate per minute, if configured
    pub overage_flat_rate_per_minute: Option<Decimal>,
    /// rate_cards.card_group to use for LPM overage tariffing when no flat rate is set
    pub overage_tariff_group: Option<String>,
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
}
