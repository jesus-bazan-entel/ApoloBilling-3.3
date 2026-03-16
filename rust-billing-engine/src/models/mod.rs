// src/models/mod.rs
pub mod account;
pub mod reservation;
pub mod rate;
pub mod cdr;

pub use account::{Account, AccountType, AccountStatus};
pub use reservation::{BalanceReservation, ReservationStatus, ReservationType};
pub use rate::RateCard;
pub use cdr::Cdr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ==================== API DTOs ====================

#[derive(Debug, Deserialize)]
pub struct AuthRequest {
    pub caller: String,
    pub callee: String,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub direction: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub authorized: bool,
    pub reason: String,
    pub uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reservation_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserved_amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_duration_seconds: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_per_minute: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct ConsumeReservationRequest {
    pub call_uuid: String,
    pub actual_cost: f64,
    pub actual_billsec: i32,
}

#[derive(Debug, Serialize)]
pub struct ConsumeReservationResponse {
    pub success: bool,
    pub total_reserved: f64,
    pub consumed: f64,
    pub released: f64,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}
