// src/models/reservation.rs
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceReservation {
    pub id: Uuid,
    pub account_id: i32,
    pub call_uuid: String,
    pub reserved_amount: Decimal,
    pub consumed_amount: Decimal,
    pub released_amount: Decimal,
    pub status: ReservationStatus,
    pub reservation_type: ReservationType,
    pub rate_per_minute: Decimal,
    pub reserved_minutes: i32,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReservationStatus {
    Active,
    PartiallyConsumed,
    FullyConsumed,
    Expired,
    Released,
    Cancelled,
}

impl ReservationStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "active" => ReservationStatus::Active,
            "partially_consumed" => ReservationStatus::PartiallyConsumed,
            "fully_consumed" => ReservationStatus::FullyConsumed,
            "expired" => ReservationStatus::Expired,
            "released" => ReservationStatus::Released,
            "cancelled" => ReservationStatus::Cancelled,
            _ => ReservationStatus::Active,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            ReservationStatus::Active => "active",
            ReservationStatus::PartiallyConsumed => "partially_consumed",
            ReservationStatus::FullyConsumed => "fully_consumed",
            ReservationStatus::Expired => "expired",
            ReservationStatus::Released => "released",
            ReservationStatus::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReservationType {
    Initial,
    Extension,
}

impl ReservationType {
    pub fn as_str(&self) -> &str {
        match self {
            ReservationType::Initial => "initial",
            ReservationType::Extension => "extension",
        }
    }
}

impl BalanceReservation {
    pub fn remaining_amount(&self) -> Decimal {
        self.reserved_amount - self.consumed_amount - self.released_amount
    }

    pub fn is_active(&self) -> bool {
        self.status == ReservationStatus::Active
    }
}
