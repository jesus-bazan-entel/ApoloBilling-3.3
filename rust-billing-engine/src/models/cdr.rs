// src/models/cdr.rs
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cdr {
    pub id: i64,
    pub uuid: String,
    pub account_id: Option<i32>,
    pub caller: String,
    pub callee: String,
    pub start_time: DateTime<Utc>,
    pub answer_time: Option<DateTime<Utc>>,
    pub end_time: DateTime<Utc>,
    pub duration: i32,
    pub billsec: i32,
    pub hangup_cause: String,
    pub rate_applied: Option<Decimal>,
    pub cost: Option<Decimal>,
    pub direction: String,
    pub freeswitch_server_id: String,
    pub reservation_id: Option<Uuid>,
}
