//! Balance reservation repository implementation
//!
//! Provides PostgreSQL-backed storage for balance reservations with
//! optimized queries for active reservation tracking and expiration.

use apolo_core::{
    models::{BalanceReservation, ReservationStatus, ReservationType},
    traits::{Repository, ReservationRepository},
    AppError, AppResult,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::{debug, error, instrument, warn};
use uuid::Uuid;

/// PostgreSQL implementation of ReservationRepository
pub struct PgReservationRepository {
    pool: PgPool,
}

impl PgReservationRepository {
    /// Create a new reservation repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Parse reservation status from string
    fn parse_status(s: &str) -> ReservationStatus {
        ReservationStatus::from_str(s).unwrap_or(ReservationStatus::Active)
    }

    /// Parse reservation type from string
    fn parse_type(s: &str) -> ReservationType {
        match s.to_lowercase().as_str() {
            "initial" => ReservationType::Initial,
            "extension" => ReservationType::Extension,
            "adjustment" => ReservationType::Adjustment,
            _ => ReservationType::Initial,
        }
    }
}

#[async_trait]
impl Repository<BalanceReservation, Uuid> for PgReservationRepository {
    #[instrument(skip(self))]
    async fn find_by_id(&self, id: Uuid) -> AppResult<Option<BalanceReservation>> {
        debug!("Finding reservation by id: {}", id);

        let result = sqlx::query_as::<sqlx::Postgres, ReservationRow>(
            r#"
            SELECT
                id, account_id, call_uuid,
                reserved_amount, consumed_amount, released_amount,
                status, reservation_type,
                destination_prefix, rate_per_minute, reserved_minutes,
                expires_at, created_at, updated_at,
                consumed_at, released_at, created_by
            FROM balance_reservations
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding reservation {}: {}", id, e);
            AppError::Database(format!("Failed to find reservation: {}", e))
        })?;

        Ok(result.map(Into::into))
    }

    #[instrument(skip(self))]
    async fn find_all(&self, limit: i64, offset: i64) -> AppResult<Vec<BalanceReservation>> {
        debug!(
            "Finding all reservations with limit {} offset {}",
            limit, offset
        );

        let rows = sqlx::query_as::<sqlx::Postgres, ReservationRow>(
            r#"
            SELECT
                id, account_id, call_uuid,
                reserved_amount, consumed_amount, released_amount,
                status, reservation_type,
                destination_prefix, rate_per_minute, reserved_minutes,
                expires_at, created_at, updated_at,
                consumed_at, released_at, created_by
            FROM balance_reservations
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding reservations: {}", e);
            AppError::Database(format!("Failed to fetch reservations: {}", e))
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    #[instrument(skip(self))]
    async fn count(&self) -> AppResult<i64> {
        let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM balance_reservations")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error counting reservations: {}", e);
                AppError::Database(format!("Failed to count reservations: {}", e))
            })?;

        Ok(result.0)
    }

    #[instrument(skip(self, entity))]
    async fn create(&self, entity: &BalanceReservation) -> AppResult<BalanceReservation> {
        debug!("Creating reservation for call: {}", entity.call_uuid);

        let row = sqlx::query_as::<sqlx::Postgres, ReservationRow>(
            r#"
            INSERT INTO balance_reservations (
                id, account_id, call_uuid,
                reserved_amount, consumed_amount, released_amount,
                status, reservation_type,
                destination_prefix, rate_per_minute, reserved_minutes,
                expires_at, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING
                id, account_id, call_uuid,
                reserved_amount, consumed_amount, released_amount,
                status, reservation_type,
                destination_prefix, rate_per_minute, reserved_minutes,
                expires_at, created_at, updated_at,
                consumed_at, released_at, created_by
            "#,
        )
        .bind(entity.id)
        .bind(entity.account_id)
        .bind(&entity.call_uuid)
        .bind(entity.reserved_amount)
        .bind(entity.consumed_amount)
        .bind(entity.released_amount)
        .bind(entity.status.to_string())
        .bind(entity.reservation_type.to_string())
        .bind(&entity.destination_prefix)
        .bind(entity.rate_per_minute)
        .bind(entity.reserved_minutes)
        .bind(entity.expires_at)
        .bind(&entity.created_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error creating reservation: {}", e);
            AppError::Database(format!("Failed to create reservation: {}", e))
        })?;

        Ok(row.into())
    }

    #[instrument(skip(self, entity))]
    async fn update(&self, entity: &BalanceReservation) -> AppResult<BalanceReservation> {
        debug!("Updating reservation: {}", entity.id);

        let row = sqlx::query_as::<sqlx::Postgres, ReservationRow>(
            r#"
            UPDATE balance_reservations
            SET account_id = $2,
                call_uuid = $3,
                reserved_amount = $4,
                consumed_amount = $5,
                released_amount = $6,
                status = $7,
                reservation_type = $8,
                destination_prefix = $9,
                rate_per_minute = $10,
                reserved_minutes = $11,
                expires_at = $12,
                consumed_at = $13,
                released_at = $14,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id, account_id, call_uuid,
                reserved_amount, consumed_amount, released_amount,
                status, reservation_type,
                destination_prefix, rate_per_minute, reserved_minutes,
                expires_at, created_at, updated_at,
                consumed_at, released_at, created_by
            "#,
        )
        .bind(entity.id)
        .bind(entity.account_id)
        .bind(&entity.call_uuid)
        .bind(entity.reserved_amount)
        .bind(entity.consumed_amount)
        .bind(entity.released_amount)
        .bind(entity.status.to_string())
        .bind(entity.reservation_type.to_string())
        .bind(&entity.destination_prefix)
        .bind(entity.rate_per_minute)
        .bind(entity.reserved_minutes)
        .bind(entity.expires_at)
        .bind(entity.consumed_at)
        .bind(entity.released_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error updating reservation {}: {}", entity.id, e);
            AppError::Database(format!("Failed to update reservation: {}", e))
        })?;

        Ok(row.into())
    }

    #[instrument(skip(self))]
    async fn delete(&self, id: Uuid) -> AppResult<bool> {
        debug!("Deleting reservation: {}", id);

        let result = sqlx::query("DELETE FROM balance_reservations WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error deleting reservation {}: {}", id, e);
                AppError::Database(format!("Failed to delete reservation: {}", e))
            })?;

        Ok(result.rows_affected() > 0)
    }
}

#[async_trait]
impl ReservationRepository for PgReservationRepository {
    #[instrument(skip(self))]
    async fn find_by_call_uuid(&self, call_uuid: &str) -> AppResult<Option<BalanceReservation>> {
        debug!("Finding reservation by call UUID: {}", call_uuid);

        let result = sqlx::query_as::<sqlx::Postgres, ReservationRow>(
            r#"
            SELECT
                id, account_id, call_uuid,
                reserved_amount, consumed_amount, released_amount,
                status, reservation_type,
                destination_prefix, rate_per_minute, reserved_minutes,
                expires_at, created_at, updated_at,
                consumed_at, released_at, created_by
            FROM balance_reservations
            WHERE call_uuid = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(call_uuid)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding reservation by call UUID: {}", e);
            AppError::Database(format!("Failed to find reservation: {}", e))
        })?;

        Ok(result.map(Into::into))
    }

    #[instrument(skip(self))]
    async fn find_active_by_account(&self, account_id: i32) -> AppResult<Vec<BalanceReservation>> {
        debug!("Finding active reservations for account: {}", account_id);

        let rows = sqlx::query_as::<sqlx::Postgres, ReservationRow>(
            r#"
            SELECT
                id, account_id, call_uuid,
                reserved_amount, consumed_amount, released_amount,
                status, reservation_type,
                destination_prefix, rate_per_minute, reserved_minutes,
                expires_at, created_at, updated_at,
                consumed_at, released_at, created_by
            FROM balance_reservations
            WHERE account_id = $1
                AND (status = 'active' OR status = 'partially_consumed')
                AND expires_at > NOW()
            ORDER BY created_at DESC
            "#,
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding active reservations: {}", e);
            AppError::Database(format!("Failed to find active reservations: {}", e))
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    #[instrument(skip(self))]
    async fn count_active_by_account(&self, account_id: i32) -> AppResult<i64> {
        debug!("Counting active reservations for account: {}", account_id);

        let result: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM balance_reservations
            WHERE account_id = $1
                AND (status = 'active' OR status = 'partially_consumed')
                AND expires_at > NOW()
            "#,
        )
        .bind(account_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error counting active reservations: {}", e);
            AppError::Database(format!("Failed to count active reservations: {}", e))
        })?;

        Ok(result.0)
    }

    #[instrument(skip(self))]
    async fn update_status(
        &self,
        id: Uuid,
        status: ReservationStatus,
        consumed: Option<Decimal>,
        released: Option<Decimal>,
    ) -> AppResult<BalanceReservation> {
        debug!("Updating reservation {} status to {}", id, status);

        let now = Utc::now();

        let row = sqlx::query_as::<sqlx::Postgres, ReservationRow>(
            r#"
            UPDATE balance_reservations
            SET status = $2,
                consumed_amount = COALESCE($3, consumed_amount),
                released_amount = COALESCE($4, released_amount),
                consumed_at = CASE
                    WHEN $3 IS NOT NULL AND consumed_at IS NULL THEN $5
                    ELSE consumed_at
                END,
                released_at = CASE
                    WHEN $4 IS NOT NULL AND released_at IS NULL THEN $5
                    ELSE released_at
                END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id, account_id, call_uuid,
                reserved_amount, consumed_amount, released_amount,
                status, reservation_type,
                destination_prefix, rate_per_minute, reserved_minutes,
                expires_at, created_at, updated_at,
                consumed_at, released_at, created_by
            "#,
        )
        .bind(id)
        .bind(status.to_string())
        .bind(consumed)
        .bind(released)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error updating reservation status: {}", e);
            AppError::Database(format!("Failed to update reservation status: {}", e))
        })?;

        Ok(row.into())
    }

    #[instrument(skip(self))]
    async fn expire_old(&self) -> AppResult<i64> {
        debug!("Expiring old reservations");

        let result = sqlx::query(
            r#"
            UPDATE balance_reservations
            SET status = 'expired',
                updated_at = NOW()
            WHERE (status = 'active' OR status = 'partially_consumed')
                AND expires_at <= NOW()
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error expiring reservations: {}", e);
            AppError::Database(format!("Failed to expire reservations: {}", e))
        })?;

        let expired_count = result.rows_affected() as i64;

        if expired_count > 0 {
            warn!("Expired {} old reservations", expired_count);
        }

        Ok(expired_count)
    }
}

/// Helper struct for mapping database rows
#[derive(Debug, sqlx::FromRow)]
struct ReservationRow {
    id: Uuid,
    account_id: i32,
    call_uuid: String,
    reserved_amount: Decimal,
    consumed_amount: Decimal,
    released_amount: Decimal,
    status: String,
    reservation_type: String,
    destination_prefix: Option<String>,
    rate_per_minute: Decimal,
    reserved_minutes: i32,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    consumed_at: Option<DateTime<Utc>>,
    released_at: Option<DateTime<Utc>>,
    created_by: Option<String>,
}

impl From<ReservationRow> for BalanceReservation {
    fn from(row: ReservationRow) -> Self {
        Self {
            id: row.id,
            account_id: row.account_id,
            call_uuid: row.call_uuid,
            reserved_amount: row.reserved_amount,
            consumed_amount: row.consumed_amount,
            released_amount: row.released_amount,
            status: PgReservationRepository::parse_status(&row.status),
            reservation_type: PgReservationRepository::parse_type(&row.reservation_type),
            destination_prefix: row.destination_prefix,
            rate_per_minute: row.rate_per_minute,
            reserved_minutes: row.reserved_minutes,
            expires_at: row.expires_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            consumed_at: row.consumed_at,
            released_at: row.released_at,
            created_by: row.created_by,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status() {
        assert_eq!(
            PgReservationRepository::parse_status("active"),
            ReservationStatus::Active
        );
        assert_eq!(
            PgReservationRepository::parse_status("released"),
            ReservationStatus::Released
        );
        assert_eq!(
            PgReservationRepository::parse_status("expired"),
            ReservationStatus::Expired
        );
    }

    #[test]
    fn test_parse_type() {
        assert_eq!(
            PgReservationRepository::parse_type("initial"),
            ReservationType::Initial
        );
        assert_eq!(
            PgReservationRepository::parse_type("extension"),
            ReservationType::Extension
        );
        assert_eq!(
            PgReservationRepository::parse_type("adjustment"),
            ReservationType::Adjustment
        );
    }
}
