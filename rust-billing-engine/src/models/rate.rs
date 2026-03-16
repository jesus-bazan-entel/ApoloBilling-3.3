// src/models/rate.rs
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateCard {
    pub id: i32,
    pub destination_prefix: String,
    pub destination_name: String,
    pub rate_per_minute: Decimal,
    pub billing_increment: i32,
    pub connection_fee: Decimal,
    pub effective_start: DateTime<Utc>,
    pub effective_end: Option<DateTime<Utc>>,
    pub priority: i32,
}

impl RateCard {
    pub fn calculate_cost(&self, billsec: i32) -> Decimal {
        // Round up to billing increment
        let rounded_billsec = if self.billing_increment > 0 {
            ((billsec + self.billing_increment - 1) / self.billing_increment) 
                * self.billing_increment
        } else {
            billsec
        };

        let minutes = Decimal::from(rounded_billsec) / Decimal::from(60);
        let duration_cost = minutes * self.rate_per_minute;
        
        duration_cost + self.connection_fee
    }

    pub fn rate_per_second(&self) -> Decimal {
        self.rate_per_minute / Decimal::from(60)
    }
}
