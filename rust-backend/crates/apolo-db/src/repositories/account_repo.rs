//! Account repository implementation
//!
//! Provides PostgreSQL-backed storage for account entities with optimized
//! queries for phone number lookups.

use apolo_core::{
    models::{Account, AccountStatus},
    traits::{AccountRepository, Repository},
    AppError, AppResult,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::{debug, error, instrument};

const ACCOUNT_COLS: &str = "id, account_number, customer_phone, status, max_concurrent_calls, \
    plan_id, bucket_type, included_minutes, proration_mode, overage_flat_rate_per_minute, \
    overage_tariff_group, notification_email, created_at, updated_at";

/// PostgreSQL implementation of AccountRepository
pub struct PgAccountRepository {
    pool: PgPool,
}

impl PgAccountRepository {
    /// Create a new account repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Convert database account status string to enum
    fn parse_account_status(s: &str) -> AccountStatus {
        AccountStatus::from_str(s).unwrap_or(AccountStatus::Active)
    }
}

#[async_trait]
impl Repository<Account, i32> for PgAccountRepository {
    #[instrument(skip(self))]
    async fn find_by_id(&self, id: i32) -> AppResult<Option<Account>> {
        debug!("Finding account by id: {}", id);

        let result = sqlx::query_as::<sqlx::Postgres, AccountRow>(&format!(
            "SELECT {ACCOUNT_COLS} FROM accounts WHERE id = $1"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding account {}: {}", id, e);
            AppError::Database(format!("Failed to find account: {}", e))
        })?;

        Ok(result.map(|row| row.into()))
    }

    #[instrument(skip(self))]
    async fn find_all(&self, limit: i64, offset: i64) -> AppResult<Vec<Account>> {
        debug!(
            "Finding all accounts with limit {} offset {}",
            limit, offset
        );

        let rows = sqlx::query_as::<sqlx::Postgres, AccountRow>(&format!(
            "SELECT {ACCOUNT_COLS} FROM accounts ORDER BY id LIMIT $1 OFFSET $2"
        ))
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding accounts: {}", e);
            AppError::Database(format!("Failed to fetch accounts: {}", e))
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    #[instrument(skip(self))]
    async fn count(&self) -> AppResult<i64> {
        let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error counting accounts: {}", e);
                AppError::Database(format!("Failed to count accounts: {}", e))
            })?;

        Ok(result.0)
    }

    #[instrument(skip(self, entity))]
    async fn create(&self, entity: &Account) -> AppResult<Account> {
        debug!("Creating account: {}", entity.account_number);

        let row = sqlx::query_as::<sqlx::Postgres, AccountRow>(&format!(
            r#"
            INSERT INTO accounts (
                account_number, customer_phone, status, max_concurrent_calls, plan_id,
                bucket_type, included_minutes, proration_mode,
                overage_flat_rate_per_minute, overage_tariff_group, notification_email
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING {ACCOUNT_COLS}
            "#
        ))
        .bind(&entity.account_number)
        .bind(&entity.customer_phone)
        .bind(entity.status.to_string())
        .bind(entity.max_concurrent_calls)
        .bind(entity.plan_id)
        .bind(&entity.bucket_type)
        .bind(entity.included_minutes)
        .bind(&entity.proration_mode)
        .bind(entity.overage_flat_rate_per_minute)
        .bind(&entity.overage_tariff_group)
        .bind(&entity.notification_email)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error creating account: {}", e);
            if e.to_string().contains("unique constraint") {
                AppError::AlreadyExists(format!("Account {} already exists", entity.account_number))
            } else {
                AppError::Database(format!("Failed to create account: {}", e))
            }
        })?;

        Ok(row.into())
    }

    #[instrument(skip(self, entity))]
    async fn update(&self, entity: &Account) -> AppResult<Account> {
        debug!("Updating account: {}", entity.id);

        let row = sqlx::query_as::<sqlx::Postgres, AccountRow>(&format!(
            r#"
            UPDATE accounts
            SET account_number = $2,
                customer_phone = $3,
                status = $4,
                max_concurrent_calls = $5,
                plan_id = $6,
                notification_email = $7,
                updated_at = NOW()
            WHERE id = $1
            RETURNING {ACCOUNT_COLS}
            "#
        ))
        .bind(entity.id)
        .bind(&entity.account_number)
        .bind(&entity.customer_phone)
        .bind(entity.status.to_string())
        .bind(entity.max_concurrent_calls)
        .bind(entity.plan_id)
        .bind(&entity.notification_email)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error updating account {}: {}", entity.id, e);
            AppError::Database(format!("Failed to update account: {}", e))
        })?;

        Ok(row.into())
    }

    #[instrument(skip(self))]
    async fn delete(&self, id: i32) -> AppResult<bool> {
        debug!("Deleting account: {}", id);

        let result = sqlx::query("DELETE FROM accounts WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error deleting account {}: {}", id, e);
                AppError::Database(format!("Failed to delete account: {}", e))
            })?;

        Ok(result.rows_affected() > 0)
    }
}

#[async_trait]
impl AccountRepository for PgAccountRepository {
    #[instrument(skip(self))]
    async fn find_by_number(&self, account_number: &str) -> AppResult<Option<Account>> {
        debug!("Finding account by number: {}", account_number);

        let normalized = Account::normalize_phone(account_number);

        let result = sqlx::query_as::<sqlx::Postgres, AccountRow>(&format!(
            "SELECT {ACCOUNT_COLS} FROM accounts WHERE account_number = $1"
        ))
        .bind(&normalized)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding account by number: {}", e);
            AppError::Database(format!("Failed to find account: {}", e))
        })?;

        Ok(result.map(Into::into))
    }

    #[instrument(skip(self))]
    async fn find_by_phone(&self, phone: &str) -> AppResult<Option<Account>> {
        debug!("Finding account by phone: {}", phone);

        let normalized = Account::normalize_phone(phone);

        let result = sqlx::query_as::<sqlx::Postgres, AccountRow>(&format!(
            "SELECT {ACCOUNT_COLS} FROM accounts WHERE customer_phone = $1"
        ))
        .bind(&normalized)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding account by phone: {}", e);
            AppError::Database(format!("Failed to find account: {}", e))
        })?;

        Ok(result.map(Into::into))
    }

    #[instrument(skip(self))]
    async fn list_filtered(
        &self,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> AppResult<(Vec<Account>, i64)> {
        debug!(
            "Listing accounts with filters: status={:?}, limit={}, offset={}",
            status, limit, offset
        );

        let rows = sqlx::query_as::<sqlx::Postgres, AccountRow>(&format!(
            r#"
            SELECT {ACCOUNT_COLS} FROM accounts
            WHERE ($1::text IS NULL OR status = $1)
            ORDER BY id
            LIMIT $2 OFFSET $3
            "#
        ))
        .bind(status)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error fetching filtered accounts: {}", e);
            AppError::Database(format!("Failed to fetch accounts: {}", e))
        })?;

        let total: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE ($1::text IS NULL OR status = $1)")
                .bind(status)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| {
                    error!("Database error counting filtered accounts: {}", e);
                    AppError::Database(format!("Failed to count accounts: {}", e))
                })?;

        Ok((rows.into_iter().map(Into::into).collect(), total.0))
    }
}

/// Helper struct for mapping database rows
#[derive(Debug, sqlx::FromRow)]
struct AccountRow {
    id: i32,
    account_number: String,
    customer_phone: Option<String>,
    status: String,
    max_concurrent_calls: i32,
    plan_id: Option<i32>,
    bucket_type: String,
    included_minutes: i32,
    proration_mode: String,
    overage_flat_rate_per_minute: Option<Decimal>,
    overage_tariff_group: Option<String>,
    notification_email: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<AccountRow> for Account {
    fn from(row: AccountRow) -> Self {
        Self {
            id: row.id,
            account_number: row.account_number,
            account_name: None, // Not in database
            customer_phone: row.customer_phone,
            status: PgAccountRepository::parse_account_status(&row.status),
            max_concurrent_calls: row.max_concurrent_calls,
            plan_id: row.plan_id,
            bucket_type: row.bucket_type,
            included_minutes: row.included_minutes,
            proration_mode: row.proration_mode,
            overage_flat_rate_per_minute: row.overage_flat_rate_per_minute,
            overage_tariff_group: row.overage_tariff_group,
            notification_email: row.notification_email,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_phone() {
        assert_eq!(Account::normalize_phone("+1-555-123-4567"), "15551234567");
        assert_eq!(Account::normalize_phone("51999888777"), "51999888777");
    }
}
