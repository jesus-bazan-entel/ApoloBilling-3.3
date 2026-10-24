//! Account repository implementation
//!
//! Provides PostgreSQL-backed storage for account entities with optimized queries
//! for phone number lookups and atomic balance updates.

use apolo_core::{
    models::{Account, AccountStats, AccountStatus, AccountType},
    traits::{AccountRepository, Repository},
    AppError, AppResult,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::{debug, error, instrument};

/// PostgreSQL implementation of AccountRepository
pub struct PgAccountRepository {
    pool: PgPool,
}

impl PgAccountRepository {
    /// Create a new account repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Convert database account type string to enum
    fn parse_account_type(s: &str) -> AccountType {
        AccountType::from_str(s).unwrap_or(AccountType::Prepaid)
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

        let result = sqlx::query_as::<sqlx::Postgres, AccountRow>(
            r#"
            SELECT
                id, account_number, customer_phone,
                account_type, balance, credit_limit, currency,
                status, max_concurrent_calls, plan_id,
                created_at, updated_at
            FROM accounts
            WHERE id = $1
            "#,
        )
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

        let rows = sqlx::query_as::<sqlx::Postgres, AccountRow>(
            r#"
            SELECT
                id, account_number, customer_phone,
                account_type, balance, credit_limit, currency,
                status, max_concurrent_calls, plan_id,
                created_at, updated_at
            FROM accounts
            ORDER BY id
            LIMIT $1 OFFSET $2
            "#,
        )
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

        let row = sqlx::query_as::<sqlx::Postgres, AccountRow>(
            r#"
            INSERT INTO accounts (
                account_number, account_name, customer_phone, account_type,
                balance, credit_limit, currency, status, max_concurrent_calls, plan_id,
                cod_ped, departamento, provincia, distrito
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING
                id, account_number, account_name, customer_phone,
                account_type, balance, credit_limit, currency,
                status, max_concurrent_calls, plan_id,
                cod_ped, departamento, provincia, distrito,
                created_at, updated_at
            "#,
        )
        .bind(&entity.account_number)
        .bind(&entity.account_name)
        .bind(&entity.customer_phone)
        .bind(entity.account_type.to_string())
        .bind(entity.balance)
        .bind(entity.credit_limit)
        .bind(&entity.currency)
        .bind(entity.status.to_string())
        .bind(entity.max_concurrent_calls)
        .bind(entity.plan_id)
        .bind(&entity.cod_ped)
        .bind(&entity.departamento)
        .bind(&entity.provincia)
        .bind(&entity.distrito)
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

        let row = sqlx::query_as::<sqlx::Postgres, AccountRow>(
            r#"
            UPDATE accounts
            SET account_number = $2,
                customer_phone = $3,
                account_type = $4,
                balance = $5,
                credit_limit = $6,
                currency = $7,
                status = $8,
                max_concurrent_calls = $9,
                plan_id = $10,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id, account_number, customer_phone,
                account_type, balance, credit_limit, currency,
                status, max_concurrent_calls, plan_id,
                created_at, updated_at
            "#,
        )
        .bind(entity.id)
        .bind(&entity.account_number)
        .bind(&entity.customer_phone)
        .bind(entity.account_type.to_string())
        .bind(entity.balance)
        .bind(entity.credit_limit)
        .bind(&entity.currency)
        .bind(entity.status.to_string())
        .bind(entity.max_concurrent_calls)
        .bind(entity.plan_id)
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

        let result = sqlx::query_as::<sqlx::Postgres, AccountRow>(
            r#"
            SELECT
                id, account_number, customer_phone,
                account_type, balance, credit_limit, currency,
                status, max_concurrent_calls, plan_id,
                created_at, updated_at
            FROM accounts
            WHERE account_number = $1
            "#,
        )
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

        let result = sqlx::query_as::<sqlx::Postgres, AccountRow>(
            r#"
            SELECT
                id, account_number, customer_phone,
                account_type, balance, credit_limit, currency,
                status, max_concurrent_calls, plan_id,
                created_at, updated_at
            FROM accounts
            WHERE customer_phone = $1
            "#,
        )
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
    async fn update_balance(&self, id: i32, amount: Decimal) -> AppResult<Decimal> {
        debug!("Updating balance for account {} by {}", id, amount);

        let result: (Decimal,) = sqlx::query_as(
            r#"
            UPDATE accounts
            SET balance = balance + $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING balance
            "#,
        )
        .bind(id)
        .bind(amount)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error updating balance for account {}: {}", id, e);
            AppError::Database(format!("Failed to update balance: {}", e))
        })?;

        Ok(result.0)
    }

    #[instrument(skip(self))]
    async fn list_filtered(
        &self,
        status: Option<&str>,
        account_type: Option<&str>,
        search: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> AppResult<(Vec<Account>, i64)> {
        debug!(
            "Listing accounts with filters: status={:?}, type={:?}, search={:?}, limit={}, offset={}",
            status, account_type, search, limit, offset
        );

        // Build dynamic query based on filters
        let mut query_str = String::from(
            r#"
            SELECT
                id, account_number, account_name, customer_phone,
                account_type, balance, credit_limit, currency,
                status, max_concurrent_calls, plan_id,
                cod_ped, departamento, provincia, distrito,
                created_at, updated_at
            FROM accounts
            WHERE 1=1
            "#,
        );

        let mut count_query = String::from("SELECT COUNT(*) FROM accounts WHERE 1=1");

        if let Some(s) = status {
            query_str.push_str(&format!(" AND status = '{}'", s));
            count_query.push_str(&format!(" AND status = '{}'", s));
        }

        if let Some(t) = account_type {
            query_str.push_str(&format!(" AND account_type = '{}'", t));
            count_query.push_str(&format!(" AND account_type = '{}'", t));
        }

        // Search by account number or phone. Sanitize input (account numbers and
        // phones are alphanumeric) to keep the inline LIKE injection-safe.
        if let Some(s) = search {
            let sanitized: String = s
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect();
            if !sanitized.is_empty() {
                let clause = format!(
                    " AND (account_number ILIKE '%{0}%' OR customer_phone ILIKE '%{0}%')",
                    sanitized
                );
                query_str.push_str(&clause);
                count_query.push_str(&clause);
            }
        }

        query_str.push_str(&format!(" ORDER BY id LIMIT {} OFFSET {}", limit, offset));

        // Get total count
        let total: (i64,) = sqlx::query_as(&count_query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error counting filtered accounts: {}", e);
                AppError::Database(format!("Failed to count accounts: {}", e))
            })?;

        // Get accounts
        let rows = sqlx::query_as::<sqlx::Postgres, AccountRow>(&query_str)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error fetching filtered accounts: {}", e);
                AppError::Database(format!("Failed to fetch accounts: {}", e))
            })?;

        Ok((rows.into_iter().map(Into::into).collect(), total.0))
    }

    #[instrument(skip(self))]
    async fn stats(&self) -> AppResult<AccountStats> {
        debug!("Computing global account statistics");

        let row: (i64, i64, i64, i64, Decimal) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*)::BIGINT AS total_accounts,
                COUNT(*) FILTER (WHERE LOWER(status) = 'active')::BIGINT AS active_accounts,
                COUNT(*) FILTER (WHERE LOWER(account_type) = 'prepaid')::BIGINT AS prepaid_accounts,
                COUNT(*) FILTER (WHERE LOWER(account_type) = 'postpaid')::BIGINT AS postpaid_accounts,
                COALESCE(SUM(balance), 0)::NUMERIC AS total_balance
            FROM accounts
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error computing account stats: {}", e);
            AppError::Database(format!("Failed to compute account stats: {}", e))
        })?;

        Ok(AccountStats {
            total_accounts: row.0,
            active_accounts: row.1,
            prepaid_accounts: row.2,
            postpaid_accounts: row.3,
            total_balance: row.4,
        })
    }
}

/// Helper struct for mapping database rows
#[derive(Debug, sqlx::FromRow)]
struct AccountRow {
    id: i32,
    account_number: String,
    #[sqlx(default)]
    account_name: Option<String>,
    customer_phone: Option<String>,
    account_type: String,
    balance: Decimal,
    credit_limit: Decimal,
    currency: String,
    status: String,
    max_concurrent_calls: i32,
    plan_id: Option<i32>,
    #[sqlx(default)]
    cod_ped: Option<String>,
    #[sqlx(default)]
    departamento: Option<String>,
    #[sqlx(default)]
    provincia: Option<String>,
    #[sqlx(default)]
    distrito: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<AccountRow> for Account {
    fn from(row: AccountRow) -> Self {
        Self {
            id: row.id,
            account_number: row.account_number,
            account_name: row.account_name,
            customer_phone: row.customer_phone,
            account_type: PgAccountRepository::parse_account_type(&row.account_type),
            balance: row.balance,
            credit_limit: row.credit_limit,
            currency: row.currency,
            status: PgAccountRepository::parse_account_status(&row.status),
            max_concurrent_calls: row.max_concurrent_calls,
            plan_id: row.plan_id,
            cod_ped: row.cod_ped,
            departamento: row.departamento,
            provincia: row.provincia,
            distrito: row.distrito,
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
