// src/services/reservation_manager.rs
use crate::models::{ConsumeReservationRequest, ConsumeReservationResponse};
use crate::database::DbPool;
use crate::cache::{RedisClient, CacheKeys};
use crate::error::BillingError;
use rust_decimal::Decimal;
use rust_decimal::prelude::{ToPrimitive, FromPrimitive};
use uuid::Uuid;
use chrono::{Utc, Duration};
use tracing::{info, warn, error};
use deadpool_postgres::Transaction;

// Configuration constants
const INITIAL_RESERVATION_MINUTES: i32 = 5;
const RESERVATION_BUFFER_PERCENT: i32 = 8;
const MIN_RESERVATION_AMOUNT: f64 = 0.30;
const MAX_RESERVATION_AMOUNT: f64 = 30.00;
const RESERVATION_TTL: i64 = 2700; // 45 minutes
const DEFAULT_MAX_CONCURRENT_CALLS: i32 = 5;

// Deficit management constants
const MAX_DEFICIT_AMOUNT: f64 = 10.00;       // Maximum allowed negative balance
const DEFICIT_WARNING_THRESHOLD: f64 = 5.00; // Warn when deficit exceeds this
const AUTO_SUSPEND_ON_DEFICIT: bool = true;  // Auto-suspend account on excessive deficit

pub struct ReservationResult {
    pub success: bool,
    pub reason: String,
    pub reservation_id: Uuid,
    pub reserved_amount: f64,
    pub max_duration_seconds: i32,
}

pub struct ExtensionResult {
    pub success: bool,
    pub reason: String,
    pub additional_reserved: f64,
    pub new_max_duration_seconds: i32,
}

/// Result of deficit check
#[derive(Debug)]
pub struct DeficitStatus {
    pub has_deficit: bool,
    pub deficit_amount: Decimal,
    pub exceeds_limit: bool,
    pub should_suspend: bool,
}

/// Record of a deficit occurrence
#[derive(Debug)]
pub struct DeficitRecord {
    pub amount: f64,
    pub reason: String,
    pub call_uuid: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct ReservationManager {
    db_pool: DbPool,
    redis: RedisClient,
}

impl ReservationManager {
    pub fn new(db_pool: DbPool, redis: RedisClient) -> Self {
        Self { db_pool, redis }
    }

    pub async fn create_reservation(
        &self,
        account_id: i64,
        call_uuid: &str,
        destination: &str,
        rate_per_minute: Decimal,
        max_concurrent_calls: Option<i32>,
    ) -> Result<ReservationResult, BillingError> {
        // Calculate amount to reserve
        let base_amount = rate_per_minute * Decimal::from(INITIAL_RESERVATION_MINUTES);
        let buffer = base_amount * Decimal::from(RESERVATION_BUFFER_PERCENT) / Decimal::from(100);
        let mut total_reservation = base_amount + buffer;

        // Apply min/max limits
        total_reservation = total_reservation.max(Decimal::from_f64(MIN_RESERVATION_AMOUNT).unwrap());
        total_reservation = total_reservation.min(Decimal::from_f64(MAX_RESERVATION_AMOUNT).unwrap());

        info!(
            "Calculating reservation: base=${}, buffer=${} ({}%), total=${}",
            base_amount, buffer, RESERVATION_BUFFER_PERCENT, total_reservation
        );

        // Check available balance
        let available_balance = self.get_available_balance(account_id).await?;

        if available_balance < total_reservation {
            warn!(
                "Insufficient balance: required ${}, available ${}",
                total_reservation, available_balance
            );
            return Ok(ReservationResult {
                success: false,
                reason: "insufficient_balance".to_string(),
                reservation_id: Uuid::nil(),
                reserved_amount: 0.0,
                max_duration_seconds: 0,
            });
        }

        // Check concurrent limits using account-specific limit
        let limit = max_concurrent_calls.unwrap_or(DEFAULT_MAX_CONCURRENT_CALLS);
        if !self.check_concurrent_limits(account_id, limit).await? {
            return Ok(ReservationResult {
                success: false,
                reason: "concurrent_limit_exceeded".to_string(),
                reservation_id: Uuid::nil(),
                reserved_amount: 0.0,
                max_duration_seconds: 0,
            });
        }

        let reservation_id = Uuid::new_v4();
        let expires_at = Utc::now() + Duration::seconds(RESERVATION_TTL);

        let account_id_i32 = account_id as i32;
        let dest_prefix = &destination[..std::cmp::min(10, destination.len())];

        let client = self.db_pool.get().await
            .map_err(|e| BillingError::Internal(e.to_string()))?;

        // Use parameterized query to prevent SQL injection
        // expires_at is TIMESTAMP WITH TIME ZONE, use DateTime<Utc> directly
        client
            .execute(
                "INSERT INTO balance_reservations
                (id, account_id, call_uuid, reserved_amount, consumed_amount, released_amount,
                status, type, destination_prefix, rate_per_minute, reserved_minutes,
                expires_at, created_by)
                VALUES ($1, $2, $3, $4, $5, $6, 'active', 'initial',
                        $7, $8, $9, $10, 'system')",
                &[
                    &reservation_id,
                    &account_id_i32,
                    &call_uuid,
                    &total_reservation,
                    &Decimal::ZERO,
                    &Decimal::ZERO,
                    &dest_prefix,
                    &rate_per_minute,
                    &INITIAL_RESERVATION_MINUTES,
                    &expires_at,
                ],
            )
            .await
            .map_err(|e| {
                error!("Failed to insert reservation: {}", e);
                BillingError::Database(e)
            })?;

        info!("Reservation inserted successfully: {}", reservation_id);

        // Cache in Redis
        let reserved_f64 = total_reservation.to_f64().unwrap_or(0.0);
        let rate_f64 = rate_per_minute.to_f64().unwrap_or(0.0);

        let cache_data = serde_json::json!({
            "account_id": account_id,
            "call_uuid": call_uuid,
            "reserved_amount": reserved_f64,
            "status": "active",
            "rate_per_minute": rate_f64,
        });

        self.redis
            .set(
                &CacheKeys::reservation(&reservation_id),
                &cache_data.to_string(),
                RESERVATION_TTL as usize,
            )
            .await?;

        self.redis
            .sadd(&CacheKeys::active_reservations(account_id), &reservation_id.to_string())
            .await?;

        // Handle zero rate (toll-free) - allow unlimited duration (capped at 1 hour)
        let max_duration_seconds = if rate_per_minute.is_zero() {
            3600 // 1 hour for toll-free calls
        } else {
            ((total_reservation / rate_per_minute) * Decimal::from(60))
                .to_i32()
                .unwrap_or(0)
        };

        info!(
            "Reservation created: {} for account {}. Amount: ${}, Max duration: {}s",
            reservation_id, account_id, reserved_f64, max_duration_seconds
        );

        Ok(ReservationResult {
            success: true,
            reason: "created".to_string(),
            reservation_id,
            reserved_amount: reserved_f64,
            max_duration_seconds,
        })
    }

    /// Consume reservation with transaction to prevent race conditions.
    ///
    /// Uses `FOR UPDATE` row locking to ensure atomic consumption.
    pub async fn consume_reservation(
        &self,
        req: &ConsumeReservationRequest,
    ) -> Result<ConsumeReservationResponse, BillingError> {
        let actual_cost = Decimal::from_f64(req.actual_cost)
            .ok_or_else(|| BillingError::InvalidRequest("Invalid cost".to_string()))?;

        // Get client and start transaction
        let mut client = self.db_pool.get().await
            .map_err(|e| BillingError::Internal(e.to_string()))?;

        let transaction = client.transaction().await
            .map_err(|e| BillingError::Database(e))?;

        // Get all active reservations with row lock (FOR UPDATE)
        let rows = transaction
            .query(
                "SELECT id, account_id, reserved_amount, consumed_amount, released_amount
                 FROM balance_reservations
                 WHERE call_uuid = $1 AND status = 'active'
                 ORDER BY created_at ASC
                 FOR UPDATE",
                &[&req.call_uuid],
            )
            .await?;

        if rows.is_empty() {
            transaction.rollback().await.ok();
            error!("No active reservations found for call {}", req.call_uuid);
            return Err(BillingError::ReservationFailed("No active reservations".to_string()));
        }

        let first_row = &rows[0];
        let account_id_i32: i32 = first_row.get(1);
        let account_id: i64 = account_id_i32 as i64;

        let mut total_reserved = Decimal::ZERO;

        for row in &rows {
            let reserved: Decimal = row.get(2);
            let consumed: Decimal = row.get(3);
            total_reserved += reserved - consumed;
        }

        info!(
            "Processing reservation consumption for call {}. Reserved: ${}, Actual: ${}",
            req.call_uuid, total_reserved, actual_cost
        );

        let (consumed, released) = if actual_cost <= total_reserved {
            // Normal case
            self.consume_normal_tx(&transaction, &rows, actual_cost, account_id, &req.call_uuid).await?;
            (actual_cost, total_reserved - actual_cost)
        } else {
            // Deficit case
            self.consume_deficit_tx(&transaction, &rows, actual_cost, total_reserved, account_id, &req.call_uuid).await?;
            (total_reserved, Decimal::ZERO)
        };

        // Commit transaction
        transaction.commit().await
            .map_err(|e| BillingError::Database(e))?;

        // Cleanup Redis AFTER successful commit
        for row in &rows {
            let reservation_id: Uuid = row.get(0);
            let _ = self.redis.delete(&CacheKeys::reservation(&reservation_id)).await;
            let _ = self.redis.srem(&CacheKeys::active_reservations(account_id), &reservation_id.to_string()).await;
        }

        info!(
            "Reservation consumed for call {}. Reserved: ${}, Consumed: ${}, Released: ${}",
            req.call_uuid, total_reserved, consumed, released
        );

        Ok(ConsumeReservationResponse {
            success: true,
            total_reserved: total_reserved.to_f64().unwrap(),
            consumed: consumed.to_f64().unwrap(),
            released: released.to_f64().unwrap(),
        })
    }

    async fn get_available_balance(&self, account_id: i64) -> Result<Decimal, BillingError> {
        let account_id_i32 = account_id as i32;

        let client = self.db_pool.get().await
            .map_err(|e| BillingError::Internal(e.to_string()))?;

        // Get balance, credit_limit, and account_type
        let account_row = client
            .query_one(
                "SELECT balance, COALESCE(credit_limit, 0), account_type FROM accounts WHERE id = $1",
                &[&account_id_i32],
            )
            .await?;
        let balance: Decimal = account_row.get(0);
        let credit_limit: Decimal = account_row.get(1);
        let account_type: String = account_row.get(2);

        let reserved_row = client
            .query_one(
                "SELECT COALESCE(SUM(reserved_amount - consumed_amount), 0)
                 FROM balance_reservations
                 WHERE account_id = $1 AND status = 'active'",
                &[&account_id_i32],
            )
            .await?;
        let total_reserved: Decimal = reserved_row.get(0);

        // For postpaid accounts, add credit_limit to available balance
        let available = if account_type.to_lowercase() == "postpaid" {
            balance + credit_limit - total_reserved
        } else {
            balance - total_reserved
        };

        info!(
            "Available balance for account {}: balance=${}, credit_limit=${}, reserved=${}, available=${} (type={})",
            account_id, balance, credit_limit, total_reserved, available, account_type
        );

        Ok(available)
    }

    async fn check_concurrent_limits(
        &self,
        account_id: i64,
        max_concurrent_calls: i32,
    ) -> Result<bool, BillingError> {
        let active_count = self.redis
            .scard(&CacheKeys::active_reservations(account_id))
            .await?;

        info!(
            "Concurrent calls check: account {} has {} active, limit {}",
            account_id, active_count, max_concurrent_calls
        );

        Ok(active_count < max_concurrent_calls)
    }

    /// Consume reservations within a transaction (normal case)
    async fn consume_normal_tx<'a>(
        &self,
        transaction: &Transaction<'a>,
        rows: &[tokio_postgres::Row],
        actual_cost: Decimal,
        account_id: i64,
        call_uuid: &str,
    ) -> Result<(), BillingError> {
        let account_id_i32 = account_id as i32;
        let mut remaining = actual_cost;

        // Consume FIFO
        for row in rows {
            if remaining <= Decimal::ZERO {
                break;
            }

            let reservation_id: Uuid = row.get(0);
            let reserved: Decimal = row.get(2);
            let consumed: Decimal = row.get(3);
            let available = reserved - consumed;

            let consume_from_this = remaining.min(available);

            transaction
                .execute(
                    "UPDATE balance_reservations
                     SET consumed_amount = consumed_amount + $1,
                         status = CASE
                             WHEN consumed_amount + $1 >= reserved_amount THEN 'fully_consumed'
                             ELSE 'partially_consumed'
                         END,
                         consumed_at = NOW()
                     WHERE id = $2",
                    &[&consume_from_this, &reservation_id],
                )
                .await?;

            remaining -= consume_from_this;
        }

        // Deduct from account balance
        transaction
            .execute(
                "UPDATE accounts SET balance = balance - $1, updated_at = NOW() WHERE id = $2",
                &[&actual_cost, &account_id_i32],
            )
            .await?;

        // Log transaction
        transaction
            .execute(
                "INSERT INTO balance_transactions
                 (account_id, amount, previous_balance, new_balance, type, reason, call_uuid)
                 SELECT $1, $2, balance + $2, balance, 'reservation_consume', $3, $4
                 FROM accounts WHERE id = $1",
                &[
                    &account_id_i32,
                    &(-actual_cost),
                    &format!("Consumed reservation for call {}", call_uuid),
                    &call_uuid,
                ],
            )
            .await?;

        Ok(())
    }

    /// Consume reservations within a transaction (deficit case)
    /// Implements comprehensive deficit management policy
    async fn consume_deficit_tx<'a>(
        &self,
        transaction: &Transaction<'a>,
        rows: &[tokio_postgres::Row],
        actual_cost: Decimal,
        total_reserved: Decimal,
        account_id: i64,
        call_uuid: &str,
    ) -> Result<(), BillingError> {
        let account_id_i32 = account_id as i32;
        let deficit = actual_cost - total_reserved;

        // Mark all as fully consumed
        for row in rows {
            let reservation_id: Uuid = row.get(0);
            let reserved: Decimal = row.get(2);

            transaction
                .execute(
                    "UPDATE balance_reservations
                     SET consumed_amount = $1,
                         status = 'fully_consumed',
                         consumed_at = NOW()
                     WHERE id = $2",
                    &[&reserved, &reservation_id],
                )
                .await?;
        }

        // Get current balance before deduction
        let balance_row = transaction
            .query_one(
                "SELECT balance FROM accounts WHERE id = $1 FOR UPDATE",
                &[&account_id_i32],
            )
            .await?;
        let current_balance: Decimal = balance_row.get(0);
        let new_balance = current_balance - actual_cost;

        // Deduct FULL cost (may result in negative balance)
        transaction
            .execute(
                "UPDATE accounts SET balance = $1, updated_at = NOW() WHERE id = $2",
                &[&new_balance, &account_id_i32],
            )
            .await?;

        // Log the deficit transaction
        let deficit_reason = format!(
            "DEFICIT: Reserved ${:.2}, Actual ${:.2}, Deficit ${:.2} for call {}",
            total_reserved, actual_cost, deficit, call_uuid
        );

        transaction
            .execute(
                "INSERT INTO balance_transactions
                 (account_id, amount, previous_balance, new_balance, type, reason, call_uuid)
                 VALUES ($1, $2, $3, $4, 'reservation_consume', $5, $6)",
                &[
                    &account_id_i32,
                    &(-actual_cost),
                    &current_balance,
                    &new_balance,
                    &deficit_reason,
                    &call_uuid,
                ],
            )
            .await?;

        // Log dedicated deficit record for tracking
        let _ = transaction
            .execute(
                "INSERT INTO balance_transactions
                 (account_id, amount, previous_balance, new_balance, type, reason, call_uuid)
                 VALUES ($1, $2, $3, $4, 'deficit_incurred', $5, $6)",
                &[
                    &account_id_i32,
                    &(-deficit),
                    &(new_balance + deficit),
                    &new_balance,
                    &format!("Deficit incurred: call exceeded reservation by ${:.2}", deficit),
                    &call_uuid,
                ],
            )
            .await;

        // Check if deficit exceeds warning threshold
        let max_deficit = Decimal::from_f64(MAX_DEFICIT_AMOUNT).unwrap_or(Decimal::from(10));
        let warning_threshold = Decimal::from_f64(DEFICIT_WARNING_THRESHOLD).unwrap_or(Decimal::from(5));

        if new_balance < Decimal::ZERO {
            let abs_deficit = -new_balance;

            if abs_deficit >= warning_threshold {
                warn!(
                    "⚠️ DEFICIT WARNING: Account {} balance is -${:.2} (threshold: ${:.2})",
                    account_id, abs_deficit, warning_threshold
                );
            }

            // Check if we should auto-suspend the account
            if abs_deficit >= max_deficit && AUTO_SUSPEND_ON_DEFICIT {
                warn!(
                    "🚨 AUTO-SUSPEND: Account {} exceeded max deficit of ${:.2}, current: -${:.2}",
                    account_id, max_deficit, abs_deficit
                );

                // Suspend the account
                let _ = transaction
                    .execute(
                        "UPDATE accounts SET status = 'SUSPENDED', updated_at = NOW() WHERE id = $1",
                        &[&account_id_i32],
                    )
                    .await;

                // Log suspension
                let _ = transaction
                    .execute(
                        "INSERT INTO balance_transactions
                         (account_id, amount, previous_balance, new_balance, type, reason, call_uuid)
                         VALUES ($1, 0, $2, $2, 'account_suspended', $3, $4)",
                        &[
                            &account_id_i32,
                            &new_balance,
                            &format!("Account suspended: deficit ${:.2} exceeded limit ${:.2}", abs_deficit, max_deficit),
                            &call_uuid,
                        ],
                    )
                    .await;

                error!(
                    "🚨 ACCOUNT SUSPENDED: Account {} auto-suspended due to excessive deficit: ${:.2}",
                    account_id, abs_deficit
                );
            }
        }

        error!(
            "RESERVATION DEFICIT: Account {}, Call {}, Deficit: ${:.2}, New Balance: ${:.2}",
            account_id, call_uuid, deficit, new_balance
        );

        Ok(())
    }

    /// Check account deficit status
    pub async fn check_deficit_status(&self, account_id: i64) -> Result<DeficitStatus, BillingError> {
        let account_id_i32 = account_id as i32;

        let client = self.db_pool.get().await
            .map_err(|e| BillingError::Internal(e.to_string()))?;

        let row = client
            .query_one("SELECT balance FROM accounts WHERE id = $1", &[&account_id_i32])
            .await?;
        let balance: Decimal = row.get(0);

        let max_deficit = Decimal::from_f64(MAX_DEFICIT_AMOUNT).unwrap_or(Decimal::from(10));

        if balance < Decimal::ZERO {
            let deficit_amount = -balance;
            Ok(DeficitStatus {
                has_deficit: true,
                deficit_amount,
                exceeds_limit: deficit_amount >= max_deficit,
                should_suspend: deficit_amount >= max_deficit && AUTO_SUSPEND_ON_DEFICIT,
            })
        } else {
            Ok(DeficitStatus {
                has_deficit: false,
                deficit_amount: Decimal::ZERO,
                exceeds_limit: false,
                should_suspend: false,
            })
        }
    }

    /// Get total deficit history for an account
    pub async fn get_deficit_history(&self, account_id: i64) -> Result<Vec<DeficitRecord>, BillingError> {
        let account_id_i32 = account_id as i32;

        let client = self.db_pool.get().await
            .map_err(|e| BillingError::Internal(e.to_string()))?;

        let rows = client
            .query(
                "SELECT amount, reason, call_uuid, created_at
                 FROM balance_transactions
                 WHERE account_id = $1 AND type = 'deficit_incurred'
                 ORDER BY created_at DESC
                 LIMIT 100",
                &[&account_id_i32],
            )
            .await?;

        let records: Vec<DeficitRecord> = rows
            .iter()
            .map(|row| {
                let amount: Decimal = row.get(0);
                DeficitRecord {
                    amount: (-amount).to_f64().unwrap_or(0.0),
                    reason: row.get(1),
                    call_uuid: row.get(2),
                    created_at: row.get(3),
                }
            })
            .collect();

        Ok(records)
    }

    /// Extend an existing reservation when call is approaching max duration
    pub async fn extend_reservation(
        &self,
        call_uuid: &str,
        additional_minutes: i32,
    ) -> Result<ExtensionResult, BillingError> {
        info!("Attempting to extend reservation for call: {}", call_uuid);

        let client = self.db_pool.get().await
            .map_err(|e| BillingError::Internal(e.to_string()))?;

        let rows = client
            .query(
                "SELECT id, account_id, rate_per_minute, reserved_amount, consumed_amount
                 FROM balance_reservations
                 WHERE call_uuid = $1 AND status = 'active'
                 ORDER BY created_at DESC
                 LIMIT 1",
                &[&call_uuid],
            )
            .await?;

        if rows.is_empty() {
            warn!("No active reservation found for call: {}", call_uuid);
            return Ok(ExtensionResult {
                success: false,
                reason: "no_active_reservation".to_string(),
                additional_reserved: 0.0,
                new_max_duration_seconds: 0,
            });
        }

        let row = &rows[0];
        let reservation_id: Uuid = row.get(0);
        let account_id: i64 = row.get(1);
        let rate_per_minute: Decimal = row.get(2);
        let current_reserved: Decimal = row.get(3);
        let consumed: Decimal = row.get(4);

        // Calculate extension amount
        let base_amount = rate_per_minute * Decimal::from(additional_minutes);
        let buffer = base_amount * Decimal::from(RESERVATION_BUFFER_PERCENT) / Decimal::from(100);
        let mut extension_amount = base_amount + buffer;

        extension_amount = extension_amount.max(Decimal::from_f64(MIN_RESERVATION_AMOUNT).unwrap());
        extension_amount = extension_amount.min(Decimal::from_f64(MAX_RESERVATION_AMOUNT).unwrap());

        info!(
            "Extension calculation: base=${}, buffer=${} ({}%), total=${}",
            base_amount, buffer, RESERVATION_BUFFER_PERCENT, extension_amount
        );

        let available_balance = self.get_available_balance(account_id).await?;

        if available_balance < extension_amount {
            warn!(
                "Insufficient balance for extension: required ${}, available ${}",
                extension_amount, available_balance
            );
            return Ok(ExtensionResult {
                success: false,
                reason: "insufficient_balance".to_string(),
                additional_reserved: 0.0,
                new_max_duration_seconds: 0,
            });
        }

        let extension_id = Uuid::new_v4();
        let expires_at = Utc::now() + Duration::seconds(RESERVATION_TTL);

        let dest_row = client
            .query_one(
                "SELECT destination_prefix FROM balance_reservations WHERE id = $1",
                &[&reservation_id],
            )
            .await?;
        let destination_prefix: String = dest_row.get(0);

        let account_id_i32 = account_id as i32;
        let call_uuid_str = call_uuid.to_string();

        // Use parameterized query
        // expires_at is TIMESTAMP WITH TIME ZONE, use DateTime<Utc> directly
        client
            .execute(
                "INSERT INTO balance_reservations
                (id, account_id, call_uuid, reserved_amount, consumed_amount, released_amount,
                status, type, destination_prefix, rate_per_minute, reserved_minutes,
                expires_at, created_by)
                VALUES ($1, $2, $3, $4, $5, $6, 'active', 'extension',
                        $7, $8, $9, $10, 'system_extension')",
                &[
                    &extension_id,
                    &account_id_i32,
                    &call_uuid_str,
                    &extension_amount,
                    &Decimal::ZERO,
                    &Decimal::ZERO,
                    &destination_prefix,
                    &rate_per_minute,
                    &additional_minutes,
                    &expires_at,
                ],
            )
            .await
            .map_err(|e| {
                error!("Failed to insert extension reservation: {}", e);
                BillingError::Database(e)
            })?;

        let cache_data = serde_json::json!({
            "account_id": account_id,
            "call_uuid": call_uuid,
            "reserved_amount": extension_amount.to_f64().unwrap(),
            "status": "active",
            "rate_per_minute": rate_per_minute.to_f64().unwrap(),
            "type": "extension",
        });

        self.redis
            .set(
                &CacheKeys::reservation(&extension_id),
                &cache_data.to_string(),
                RESERVATION_TTL as usize,
            )
            .await?;

        self.redis
            .sadd(&CacheKeys::active_reservations(account_id), &extension_id.to_string())
            .await?;

        let total_reserved = current_reserved + extension_amount - consumed;
        // Handle zero rate (toll-free) - allow unlimited duration (capped at 1 hour)
        let new_max_duration_seconds = if rate_per_minute.is_zero() {
            3600 // 1 hour for toll-free calls
        } else {
            ((total_reserved / rate_per_minute) * Decimal::from(60))
                .to_i32()
                .unwrap_or(0)
        };

        info!(
            "Reservation extended: {} for call {}. Extension: ${}, New max duration: {}s",
            extension_id, call_uuid, extension_amount, new_max_duration_seconds
        );

        Ok(ExtensionResult {
            success: true,
            reason: "extended".to_string(),
            additional_reserved: extension_amount.to_f64().unwrap(),
            new_max_duration_seconds,
        })
    }
}
