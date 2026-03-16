//! Balance reservation and transaction models
//!
//! Manages balance holds during calls and transaction history.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Reservation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReservationStatus {
    /// Reservation is active and holding balance
    #[default]
    Active,
    /// Part of the reservation has been consumed
    PartiallyConsumed,
    /// All reserved amount has been consumed
    FullyConsumed,
    /// Reservation was released without consumption
    Released,
    /// Reservation expired (call didn't complete in time)
    Expired,
    /// Reservation was cancelled
    Cancelled,
}

impl fmt::Display for ReservationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReservationStatus::Active => write!(f, "active"),
            ReservationStatus::PartiallyConsumed => write!(f, "partially_consumed"),
            ReservationStatus::FullyConsumed => write!(f, "fully_consumed"),
            ReservationStatus::Released => write!(f, "released"),
            ReservationStatus::Expired => write!(f, "expired"),
            ReservationStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl ReservationStatus {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "active" => Some(ReservationStatus::Active),
            "partially_consumed" => Some(ReservationStatus::PartiallyConsumed),
            "fully_consumed" => Some(ReservationStatus::FullyConsumed),
            "released" => Some(ReservationStatus::Released),
            "expired" => Some(ReservationStatus::Expired),
            "cancelled" => Some(ReservationStatus::Cancelled),
            _ => None,
        }
    }

    /// Check if reservation is still holding balance
    pub fn is_holding(&self) -> bool {
        matches!(
            self,
            ReservationStatus::Active | ReservationStatus::PartiallyConsumed
        )
    }

    /// Check if reservation is finalized
    pub fn is_final(&self) -> bool {
        !self.is_holding()
    }
}

/// Reservation type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReservationType {
    /// Initial reservation at call start
    #[default]
    Initial,
    /// Extension reservation during call
    Extension,
    /// Manual adjustment
    Adjustment,
}

impl fmt::Display for ReservationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReservationType::Initial => write!(f, "initial"),
            ReservationType::Extension => write!(f, "extension"),
            ReservationType::Adjustment => write!(f, "adjustment"),
        }
    }
}

/// Balance reservation entity
///
/// Represents a hold on account balance during a call.
/// The reservation lifecycle:
/// 1. Created at call authorization (Active)
/// 2. Partially consumed as call progresses
/// 3. Fully consumed or released at call end
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceReservation {
    /// Unique identifier (UUID)
    pub id: Uuid,

    /// Associated account ID
    pub account_id: i32,

    /// Associated call UUID
    pub call_uuid: String,

    /// Total amount reserved
    pub reserved_amount: Decimal,

    /// Amount consumed so far
    pub consumed_amount: Decimal,

    /// Amount released back to account
    pub released_amount: Decimal,

    /// Current status
    pub status: ReservationStatus,

    /// Type of reservation
    pub reservation_type: ReservationType,

    /// Matched destination prefix
    pub destination_prefix: Option<String>,

    /// Rate per minute at time of reservation
    pub rate_per_minute: Decimal,

    /// Estimated minutes based on reservation
    pub reserved_minutes: i32,

    /// When reservation expires
    pub expires_at: DateTime<Utc>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// When consumption occurred
    pub consumed_at: Option<DateTime<Utc>>,

    /// When release occurred
    pub released_at: Option<DateTime<Utc>>,

    /// Who created this reservation
    pub created_by: Option<String>,
}

impl BalanceReservation {
    /// Create a new reservation
    pub fn new(
        account_id: i32,
        call_uuid: String,
        reserved_amount: Decimal,
        rate_per_minute: Decimal,
        reserved_minutes: i32,
        ttl_seconds: i64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            account_id,
            call_uuid,
            reserved_amount,
            consumed_amount: Decimal::ZERO,
            released_amount: Decimal::ZERO,
            status: ReservationStatus::Active,
            reservation_type: ReservationType::Initial,
            destination_prefix: None,
            rate_per_minute,
            reserved_minutes,
            expires_at: now + chrono::Duration::seconds(ttl_seconds),
            created_at: now,
            updated_at: now,
            consumed_at: None,
            released_at: None,
            created_by: None,
        }
    }

    /// Get remaining balance in reservation
    #[inline]
    pub fn remaining(&self) -> Decimal {
        self.reserved_amount - self.consumed_amount - self.released_amount
    }

    /// Check if reservation is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if reservation can accept more consumption
    pub fn can_consume(&self) -> bool {
        self.status.is_holding() && !self.is_expired() && self.remaining() > Decimal::ZERO
    }

    /// Calculate maximum allowed duration based on reserved amount and rate
    pub fn max_duration_seconds(&self) -> i32 {
        if self.rate_per_minute <= Decimal::ZERO {
            return i32::MAX;
        }

        let minutes = self.reserved_amount / self.rate_per_minute;
        (minutes * Decimal::from(60))
            .to_string()
            .parse()
            .unwrap_or(300)
    }
}

impl Default for BalanceReservation {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            account_id: 0,
            call_uuid: String::new(),
            reserved_amount: Decimal::ZERO,
            consumed_amount: Decimal::ZERO,
            released_amount: Decimal::ZERO,
            status: ReservationStatus::Active,
            reservation_type: ReservationType::Initial,
            destination_prefix: None,
            rate_per_minute: Decimal::ZERO,
            reserved_minutes: 0,
            expires_at: now + chrono::Duration::minutes(45),
            created_at: now,
            updated_at: now,
            consumed_at: None,
            released_at: None,
            created_by: None,
        }
    }
}

/// Transaction type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    /// Credit added to account (recharge)
    Credit,
    /// Debit from account (manual)
    Debit,
    /// Reservation created (hold)
    ReservationCreate,
    /// Reservation consumed (actual charge)
    ReservationConsume,
    /// Reservation released (unused hold returned)
    ReservationRelease,
    /// Manual adjustment
    Adjustment,
    /// Refund issued
    Refund,
    /// Deficit incurred (negative balance)
    DeficitIncurred,
}

impl fmt::Display for TransactionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactionType::Credit => write!(f, "credit"),
            TransactionType::Debit => write!(f, "debit"),
            TransactionType::ReservationCreate => write!(f, "reservation_create"),
            TransactionType::ReservationConsume => write!(f, "reservation_consume"),
            TransactionType::ReservationRelease => write!(f, "reservation_release"),
            TransactionType::Adjustment => write!(f, "adjustment"),
            TransactionType::Refund => write!(f, "refund"),
            TransactionType::DeficitIncurred => write!(f, "deficit_incurred"),
        }
    }
}

/// Balance transaction entity
///
/// Immutable audit log of all balance changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceTransaction {
    /// Unique identifier
    pub id: i64,

    /// Associated account ID
    pub account_id: i32,

    /// Transaction amount (positive or negative)
    pub amount: Decimal,

    /// Balance before transaction
    pub previous_balance: Decimal,

    /// Balance after transaction
    pub new_balance: Decimal,

    /// Type of transaction
    pub transaction_type: TransactionType,

    /// Reason/description
    pub reason: Option<String>,

    /// Associated call UUID (if applicable)
    pub call_uuid: Option<String>,

    /// Associated reservation ID (if applicable)
    pub reservation_id: Option<Uuid>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Who created this transaction
    pub created_by: Option<String>,
}

impl BalanceTransaction {
    /// Create a new transaction record
    pub fn new(
        account_id: i32,
        amount: Decimal,
        previous_balance: Decimal,
        transaction_type: TransactionType,
        reason: Option<String>,
    ) -> Self {
        Self {
            id: 0,
            account_id,
            amount,
            previous_balance,
            new_balance: previous_balance + amount,
            transaction_type,
            reason,
            call_uuid: None,
            reservation_id: None,
            created_at: Utc::now(),
            created_by: None,
        }
    }

    /// Check if this is a debit transaction (reduces balance)
    pub fn is_debit(&self) -> bool {
        self.amount < Decimal::ZERO
    }

    /// Check if this is a credit transaction (increases balance)
    pub fn is_credit(&self) -> bool {
        self.amount > Decimal::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_reservation_remaining() {
        let mut res = BalanceReservation {
            reserved_amount: dec!(10.00),
            consumed_amount: dec!(3.00),
            released_amount: dec!(2.00),
            ..Default::default()
        };

        assert_eq!(res.remaining(), dec!(5.00));

        res.consumed_amount = dec!(8.00);
        assert_eq!(res.remaining(), dec!(0.00));
    }

    #[test]
    fn test_reservation_status() {
        assert!(ReservationStatus::Active.is_holding());
        assert!(ReservationStatus::PartiallyConsumed.is_holding());
        assert!(!ReservationStatus::FullyConsumed.is_holding());
        assert!(!ReservationStatus::Released.is_holding());
    }

    #[test]
    fn test_transaction_new() {
        let tx = BalanceTransaction::new(
            1,
            dec!(50.00),
            dec!(100.00),
            TransactionType::Credit,
            Some("Recharge".to_string()),
        );

        assert_eq!(tx.previous_balance, dec!(100.00));
        assert_eq!(tx.new_balance, dec!(150.00));
        assert!(tx.is_credit());
    }

    #[test]
    fn test_transaction_debit() {
        let tx = BalanceTransaction::new(
            1,
            dec!(-25.00),
            dec!(100.00),
            TransactionType::ReservationConsume,
            None,
        );

        assert_eq!(tx.new_balance, dec!(75.00));
        assert!(tx.is_debit());
    }
}
