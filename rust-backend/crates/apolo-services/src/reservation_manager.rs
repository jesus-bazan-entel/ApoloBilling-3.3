//! Reservation manager service
//!
//! Manages balance reservations throughout the call lifecycle:
//! - Create reservations at call start
//! - Extend reservations for long calls
//! - Consume reservations when call ends
//! - Release unused reservations
//! - Handle deficit scenarios (negative balance)

use apolo_core::{
    models::{Account, BalanceReservation, RateCard, ReservationStatus},
    traits::{AccountRepository, ReservationRepository},
    AppError, AppResult,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};

use crate::constants::{
    INITIAL_RESERVATION_MINUTES, MAX_DEFICIT, MAX_RESERVATION, MIN_RESERVATION,
    RESERVATION_BUFFER_PERCENT, RESERVATION_TTL,
};

/// Reservation manager
///
/// Handles all balance reservation operations with proper transaction management
/// and deficit handling.
pub struct ReservationManager<A: AccountRepository, R: ReservationRepository> {
    account_repo: Arc<A>,
    reservation_repo: Arc<R>,
    pool: Arc<PgPool>,
}

impl<A: AccountRepository, R: ReservationRepository> ReservationManager<A, R> {
    /// Create a new reservation manager
    pub fn new(account_repo: Arc<A>, reservation_repo: Arc<R>, pool: Arc<PgPool>) -> Self {
        Self {
            account_repo,
            reservation_repo,
            pool,
        }
    }

    /// Calculate reservation amount with buffer
    fn calculate_reservation_amount(rate: &RateCard, minutes: i32) -> Decimal {
        let base_cost = rate.estimate_cost_minutes(minutes);
        let buffer = base_cost * Decimal::from(RESERVATION_BUFFER_PERCENT) / Decimal::from(100);
        let total = base_cost + buffer;

        // Clamp to min/max limits
        total.max(MIN_RESERVATION).min(MAX_RESERVATION)
    }

    /// Create a new balance reservation
    ///
    /// # Arguments
    ///
    /// * `account_id` - Account to reserve balance from
    /// * `call_uuid` - Unique call identifier
    /// * `rate` - Rate card to use for calculation
    /// * `minutes` - Number of minutes to reserve (default: INITIAL_RESERVATION_MINUTES)
    ///
    /// # Returns
    ///
    /// The created reservation
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Account doesn't have sufficient balance
    /// - Database transaction fails
    #[instrument(skip(self, rate))]
    pub async fn create_reservation(
        &self,
        account_id: i32,
        call_uuid: String,
        rate: &RateCard,
        minutes: Option<i32>,
    ) -> AppResult<BalanceReservation> {
        let minutes = minutes.unwrap_or(INITIAL_RESERVATION_MINUTES);
        let reserved_amount = Self::calculate_reservation_amount(rate, minutes);

        info!(
            "Creating reservation for account {}, call {}: {} for {} minutes",
            account_id, call_uuid, reserved_amount, minutes
        );

        // Start transaction
        let mut tx = self.pool.begin().await.map_err(|e| {
            error!("Failed to start transaction: {}", e);
            AppError::Transaction(format!("Failed to start transaction: {}", e))
        })?;

        // Lock account row
        let account = sqlx::query_as::<sqlx::Postgres, AccountRow>(
            r#"
            SELECT id, account_number, customer_phone, account_type,
                   balance, credit_limit, currency, status, max_concurrent_calls,
                   plan_id, created_at, updated_at
            FROM accounts
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(account_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to lock account: {}", e);
            AppError::Database(format!("Failed to lock account: {}", e))
        })?
        .ok_or_else(|| AppError::AccountNotFound(account_id.to_string()))?;

        let account: Account = account.into();

        // Check if account can authorize this amount
        let available = account.available_balance();
        if available < reserved_amount {
            warn!(
                "Insufficient balance for account {}: required {}, available {}",
                account_id, reserved_amount, available
            );
            return Err(AppError::InsufficientBalance {
                required: reserved_amount.to_string(),
                available: available.to_string(),
            });
        }

        // Deduct balance (create reservation hold)
        sqlx::query(
            r#"
            UPDATE accounts
            SET balance = balance - $2,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(account_id)
        .bind(reserved_amount)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to update account balance: {}", e);
            AppError::Database(format!("Failed to update balance: {}", e))
        })?;

        // Create reservation record
        let reservation = BalanceReservation::new(
            account_id,
            call_uuid.clone(),
            reserved_amount,
            rate.rate_per_minute,
            minutes,
            RESERVATION_TTL,
        );

        sqlx::query(
            r#"
            INSERT INTO balance_reservations (
                id, account_id, call_uuid, reserved_amount, consumed_amount,
                released_amount, status, reservation_type, destination_prefix,
                rate_per_minute, reserved_minutes, expires_at, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(reservation.id)
        .bind(reservation.account_id)
        .bind(&reservation.call_uuid)
        .bind(reservation.reserved_amount)
        .bind(reservation.consumed_amount)
        .bind(reservation.released_amount)
        .bind(reservation.status.to_string())
        .bind(reservation.reservation_type.to_string())
        .bind(&rate.destination_prefix)
        .bind(reservation.rate_per_minute)
        .bind(reservation.reserved_minutes)
        .bind(reservation.expires_at)
        .bind(&reservation.created_by)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to create reservation: {}", e);
            AppError::Database(format!("Failed to create reservation: {}", e))
        })?;

        // Commit transaction
        tx.commit().await.map_err(|e| {
            error!("Failed to commit transaction: {}", e);
            AppError::Transaction(format!("Failed to commit transaction: {}", e))
        })?;

        info!(
            "Created reservation {} for account {}: {}",
            reservation.id, account_id, reserved_amount
        );

        Ok(reservation)
    }

    /// Consume a reservation (charge for actual call duration)
    ///
    /// # Arguments
    ///
    /// * `call_uuid` - Call identifier
    /// * `actual_cost` - Actual cost to charge
    /// * `actual_billsec` - Actual billable seconds
    ///
    /// # Returns
    ///
    /// The amount released back to account (reserved - consumed)
    ///
    /// # Deficit Handling
    ///
    /// If actual_cost > reserved_amount, creates deficit up to MAX_DEFICIT limit
    #[instrument(skip(self))]
    pub async fn consume_reservation(
        &self,
        call_uuid: &str,
        actual_cost: Decimal,
        actual_billsec: i32,
    ) -> AppResult<Decimal> {
        info!(
            "Consuming reservation for call {}: cost={}, billsec={}",
            call_uuid, actual_cost, actual_billsec
        );

        // Start transaction
        let mut tx = self.pool.begin().await.map_err(|e| {
            error!("Failed to start transaction: {}", e);
            AppError::Transaction(format!("Failed to start transaction: {}", e))
        })?;

        // Find and lock reservation
        let reservation = self
            .reservation_repo
            .find_by_call_uuid(call_uuid)
            .await?
            .ok_or_else(|| AppError::ReservationNotFound(call_uuid.to_string()))?;

        if !reservation.status.is_holding() {
            return Err(AppError::ReservationFailed(format!(
                "Reservation {} is not active (status: {})",
                reservation.id, reservation.status
            )));
        }

        let reserved = reservation.reserved_amount;
        let mut consumed = actual_cost;
        let mut released = Decimal::ZERO;
        let mut deficit = Decimal::ZERO;

        // Calculate consumption
        if actual_cost <= reserved {
            // Normal case: actual cost fits within reservation
            released = reserved - actual_cost;
            debug!(
                "Normal consumption: reserved={}, consumed={}, released={}",
                reserved, consumed, released
            );
        } else {
            // Deficit case: actual cost exceeds reservation
            deficit = actual_cost - reserved;
            consumed = reserved;
            released = Decimal::ZERO;

            warn!(
                "Deficit detected for call {}: deficit={}, reserved={}",
                call_uuid, deficit, reserved
            );

            // Check deficit limit
            if deficit > MAX_DEFICIT {
                error!("Deficit exceeds maximum allowed: {} > {}", deficit, MAX_DEFICIT);
                return Err(AppError::ReservationFailed(format!(
                    "Deficit {} exceeds maximum allowed {}",
                    deficit, MAX_DEFICIT
                )));
            }

            // Deduct additional deficit from account
            sqlx::query(
                r#"
                UPDATE accounts
                SET balance = balance - $2,
                    updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(reservation.account_id)
            .bind(deficit)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to apply deficit: {}", e);
                AppError::Database(format!("Failed to apply deficit: {}", e))
            })?;
        }

        // Release unused amount back to account
        if released > Decimal::ZERO {
            sqlx::query(
                r#"
                UPDATE accounts
                SET balance = balance + $2,
                    updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(reservation.account_id)
            .bind(released)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to release balance: {}", e);
                AppError::Database(format!("Failed to release balance: {}", e))
            })?;
        }

        // Update reservation status
        let new_status = if consumed >= reserved {
            ReservationStatus::FullyConsumed
        } else {
            ReservationStatus::PartiallyConsumed
        };

        self.reservation_repo
            .update_status(reservation.id, new_status, Some(consumed), Some(released))
            .await?;

        // Commit transaction
        tx.commit().await.map_err(|e| {
            error!("Failed to commit transaction: {}", e);
            AppError::Transaction(format!("Failed to commit transaction: {}", e))
        })?;

        info!(
            "Consumed reservation {}: consumed={}, released={}, deficit={}",
            reservation.id, consumed, released, deficit
        );

        Ok(released)
    }

    /// Release a reservation without consumption
    ///
    /// Returns the full reserved amount to the account
    #[instrument(skip(self))]
    pub async fn release_reservation(&self, call_uuid: &str) -> AppResult<Decimal> {
        info!("Releasing reservation for call {}", call_uuid);

        // Start transaction
        let mut tx = self.pool.begin().await.map_err(|e| {
            error!("Failed to start transaction: {}", e);
            AppError::Transaction(format!("Failed to start transaction: {}", e))
        })?;

        // Find reservation
        let reservation = self
            .reservation_repo
            .find_by_call_uuid(call_uuid)
            .await?
            .ok_or_else(|| AppError::ReservationNotFound(call_uuid.to_string()))?;

        if !reservation.status.is_holding() {
            warn!(
                "Reservation {} is not active, skipping release",
                reservation.id
            );
            return Ok(Decimal::ZERO);
        }

        let amount_to_release = reservation.remaining();

        if amount_to_release > Decimal::ZERO {
            // Return balance to account
            sqlx::query(
                r#"
                UPDATE accounts
                SET balance = balance + $2,
                    updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(reservation.account_id)
            .bind(amount_to_release)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to release balance: {}", e);
                AppError::Database(format!("Failed to release balance: {}", e))
            })?;
        }

        // Update reservation status
        self.reservation_repo
            .update_status(
                reservation.id,
                ReservationStatus::Released,
                None,
                Some(amount_to_release),
            )
            .await?;

        // Commit transaction
        tx.commit().await.map_err(|e| {
            error!("Failed to commit transaction: {}", e);
            AppError::Transaction(format!("Failed to commit transaction: {}", e))
        })?;

        info!(
            "Released reservation {}: amount={}",
            reservation.id, amount_to_release
        );

        Ok(amount_to_release)
    }

    /// Extend a reservation for longer calls
    ///
    /// # Arguments
    ///
    /// * `call_uuid` - Call identifier
    /// * `additional_minutes` - Additional minutes to reserve
    ///
    /// # Returns
    ///
    /// The additional amount reserved
    #[instrument(skip(self))]
    pub async fn extend_reservation(
        &self,
        call_uuid: &str,
        additional_minutes: i32,
    ) -> AppResult<Decimal> {
        info!(
            "Extending reservation for call {} by {} minutes",
            call_uuid, additional_minutes
        );

        // Find reservation
        let reservation = self
            .reservation_repo
            .find_by_call_uuid(call_uuid)
            .await?
            .ok_or_else(|| AppError::ReservationNotFound(call_uuid.to_string()))?;

        if !reservation.status.is_holding() {
            return Err(AppError::ReservationFailed(format!(
                "Cannot extend non-active reservation {}",
                reservation.id
            )));
        }

        // Calculate additional amount
        let rate_per_minute = reservation.rate_per_minute;
        let additional_amount = rate_per_minute * Decimal::from(additional_minutes);

        // Start transaction
        let mut tx = self.pool.begin().await.map_err(|e| {
            error!("Failed to start transaction: {}", e);
            AppError::Transaction(format!("Failed to start transaction: {}", e))
        })?;

        // Lock and check account balance
        let account = sqlx::query_as::<sqlx::Postgres, AccountRow>(
            r#"
            SELECT id, account_number, customer_phone, account_type,
                   balance, credit_limit, currency, status, max_concurrent_calls,
                   plan_id, created_at, updated_at
            FROM accounts
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(reservation.account_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| AppError::AccountNotFound(reservation.account_id.to_string()))?;

        let account: Account = account.into();

        if account.available_balance() < additional_amount {
            return Err(AppError::InsufficientBalance {
                required: additional_amount.to_string(),
                available: account.available_balance().to_string(),
            });
        }

        // Deduct additional amount
        sqlx::query(
            r#"
            UPDATE accounts
            SET balance = balance - $2,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(reservation.account_id)
        .bind(additional_amount)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        // Update reservation
        let new_reserved = reservation.reserved_amount + additional_amount;
        let new_minutes = reservation.reserved_minutes + additional_minutes;

        sqlx::query(
            r#"
            UPDATE balance_reservations
            SET reserved_amount = $2,
                reserved_minutes = $3,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(reservation.id)
        .bind(new_reserved)
        .bind(new_minutes)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?;

        // Commit transaction
        tx.commit().await.map_err(|e| {
            error!("Failed to commit transaction: {}", e);
            AppError::Transaction(format!("Failed to commit transaction: {}", e))
        })?;

        info!(
            "Extended reservation {} by {} minutes: additional_amount={}",
            reservation.id, additional_minutes, additional_amount
        );

        Ok(additional_amount)
    }
}

/// Helper struct for account row mapping
#[derive(Debug, sqlx::FromRow)]
struct AccountRow {
    id: i32,
    account_number: String,
    customer_phone: Option<String>,
    account_type: String,
    balance: Decimal,
    credit_limit: Decimal,
    currency: String,
    status: String,
    max_concurrent_calls: i32,
    plan_id: Option<i32>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<AccountRow> for Account {
    fn from(row: AccountRow) -> Self {
        use apolo_core::models::{AccountStatus, AccountType};

        Self {
            id: row.id,
            account_number: row.account_number,
            account_name: None,
            customer_phone: row.customer_phone,
            account_type: AccountType::from_str(&row.account_type).unwrap_or(AccountType::Prepaid),
            balance: row.balance,
            credit_limit: row.credit_limit,
            currency: row.currency,
            status: AccountStatus::from_str(&row.status).unwrap_or(AccountStatus::Active),
            max_concurrent_calls: row.max_concurrent_calls,
            plan_id: row.plan_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;

    #[test]
    fn test_calculate_reservation_amount() {
        let rate = RateCard {
            id: 1,
            rate_name: Some("Test".to_string()),
            destination_prefix: "51".to_string(),
            destination_name: "Peru".to_string(),
            rate_per_minute: dec!(0.10),
            billing_increment: 6,
            connection_fee: dec!(0.00),
            effective_start: Utc::now(),
            effective_end: None,
            priority: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // 5 minutes at $0.10/min = $0.50
        // With 8% buffer = $0.54
        let amount =
            ReservationManager::<apolo_db::PgAccountRepository, apolo_db::PgReservationRepository>::calculate_reservation_amount(&rate, 5);

        assert!(amount >= dec!(0.50));
        assert!(amount <= dec!(1.00));
    }
}
