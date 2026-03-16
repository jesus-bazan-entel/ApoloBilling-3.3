//! Plan repository implementation
//!
//! Provides PostgreSQL-backed storage for plan entities.

use apolo_core::{models::{AccountType, Plan}, AppError, AppResult};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, PgPool};
use tracing::{debug, error, instrument};

/// Database row representation of a plan
#[derive(Debug, FromRow)]
struct PlanRow {
    id: i32,
    plan_name: String,
    plan_code: String,
    account_type: String,
    initial_balance: Decimal,
    credit_limit: Decimal,
    max_concurrent_calls: i32,
    description: Option<String>,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    created_by: String,
}

impl From<PlanRow> for Plan {
    fn from(row: PlanRow) -> Self {
        Plan {
            id: row.id,
            plan_name: row.plan_name,
            plan_code: row.plan_code,
            account_type: AccountType::from_str(&row.account_type).unwrap_or(AccountType::Prepaid),
            initial_balance: row.initial_balance,
            credit_limit: row.credit_limit,
            max_concurrent_calls: row.max_concurrent_calls,
            description: row.description,
            enabled: row.enabled,
            created_at: row.created_at,
            updated_at: row.updated_at,
            created_by: row.created_by,
        }
    }
}

/// PostgreSQL implementation of Plan repository
pub struct PgPlanRepository {
    pool: PgPool,
}

impl PgPlanRepository {
    /// Create a new plan repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Find plan by ID
    #[instrument(skip(self))]
    pub async fn find_by_id(&self, id: i32) -> AppResult<Option<Plan>> {
        debug!("Finding plan by id: {}", id);

        let result = sqlx::query_as::<sqlx::Postgres, PlanRow>(
            r#"
            SELECT
                id, plan_name, plan_code, account_type,
                initial_balance, credit_limit, max_concurrent_calls,
                description, enabled, created_at, updated_at, created_by
            FROM plans
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding plan {}: {}", id, e);
            AppError::Database(format!("Failed to find plan: {}", e))
        })?;

        Ok(result.map(Into::into))
    }

    /// Find plan by plan code
    #[instrument(skip(self))]
    pub async fn find_by_code(&self, plan_code: &str) -> AppResult<Option<Plan>> {
        debug!("Finding plan by code: {}", plan_code);

        let result = sqlx::query_as::<sqlx::Postgres, PlanRow>(
            r#"
            SELECT
                id, plan_name, plan_code, account_type,
                initial_balance, credit_limit, max_concurrent_calls,
                description, enabled, created_at, updated_at, created_by
            FROM plans
            WHERE plan_code = $1
            "#,
        )
        .bind(plan_code)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding plan by code {}: {}", plan_code, e);
            AppError::Database(format!("Failed to find plan by code: {}", e))
        })?;

        Ok(result.map(Into::into))
    }

    /// List all plans
    #[instrument(skip(self))]
    pub async fn list_all(&self) -> AppResult<Vec<Plan>> {
        debug!("Listing all plans");

        let rows = sqlx::query_as::<sqlx::Postgres, PlanRow>(
            r#"
            SELECT
                id, plan_name, plan_code, account_type,
                initial_balance, credit_limit, max_concurrent_calls,
                description, enabled, created_at, updated_at, created_by
            FROM plans
            ORDER BY account_type, plan_name
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error listing plans: {}", e);
            AppError::Database(format!("Failed to list plans: {}", e))
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// List only active/enabled plans
    #[instrument(skip(self))]
    pub async fn list_active(&self) -> AppResult<Vec<Plan>> {
        debug!("Listing active plans");

        let rows = sqlx::query_as::<sqlx::Postgres, PlanRow>(
            r#"
            SELECT
                id, plan_name, plan_code, account_type,
                initial_balance, credit_limit, max_concurrent_calls,
                description, enabled, created_at, updated_at, created_by
            FROM plans
            WHERE enabled = true
            ORDER BY account_type, plan_name
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error listing active plans: {}", e);
            AppError::Database(format!("Failed to list active plans: {}", e))
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Create a new plan
    #[instrument(skip(self))]
    pub async fn create(
        &self,
        plan_name: &str,
        plan_code: &str,
        account_type: &AccountType,
        initial_balance: Decimal,
        credit_limit: Decimal,
        max_concurrent_calls: i32,
        description: Option<&str>,
        enabled: bool,
        created_by: &str,
    ) -> AppResult<Plan> {
        debug!("Creating plan: {}", plan_code);

        let row = sqlx::query_as::<sqlx::Postgres, PlanRow>(
            r#"
            INSERT INTO plans (
                plan_name, plan_code, account_type,
                initial_balance, credit_limit, max_concurrent_calls,
                description, enabled, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING
                id, plan_name, plan_code, account_type,
                initial_balance, credit_limit, max_concurrent_calls,
                description, enabled, created_at, updated_at, created_by
            "#,
        )
        .bind(plan_name)
        .bind(plan_code)
        .bind(account_type.to_string())
        .bind(initial_balance)
        .bind(credit_limit)
        .bind(max_concurrent_calls)
        .bind(description)
        .bind(enabled)
        .bind(created_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error creating plan: {}", e);
            if e.to_string().contains("unique constraint") {
                AppError::AlreadyExists(format!("Plan with code {} already exists", plan_code))
            } else {
                AppError::Database(format!("Failed to create plan: {}", e))
            }
        })?;

        Ok(row.into())
    }

    /// Update an existing plan
    #[instrument(skip(self))]
    pub async fn update(
        &self,
        id: i32,
        plan_name: Option<&str>,
        initial_balance: Option<Decimal>,
        credit_limit: Option<Decimal>,
        max_concurrent_calls: Option<i32>,
        description: Option<&str>,
        enabled: Option<bool>,
    ) -> AppResult<Plan> {
        debug!("Updating plan: {}", id);

        // Build dynamic update query
        let mut query = String::from("UPDATE plans SET updated_at = NOW()");
        let mut bind_count = 1;

        if plan_name.is_some() {
            query.push_str(&format!(", plan_name = ${}", bind_count));
            bind_count += 1;
        }
        if initial_balance.is_some() {
            query.push_str(&format!(", initial_balance = ${}", bind_count));
            bind_count += 1;
        }
        if credit_limit.is_some() {
            query.push_str(&format!(", credit_limit = ${}", bind_count));
            bind_count += 1;
        }
        if max_concurrent_calls.is_some() {
            query.push_str(&format!(", max_concurrent_calls = ${}", bind_count));
            bind_count += 1;
        }
        if description.is_some() {
            query.push_str(&format!(", description = ${}", bind_count));
            bind_count += 1;
        }
        if enabled.is_some() {
            query.push_str(&format!(", enabled = ${}", bind_count));
            bind_count += 1;
        }

        query.push_str(&format!(
            " WHERE id = ${} RETURNING id, plan_name, plan_code, account_type, \
             initial_balance, credit_limit, max_concurrent_calls, \
             description, enabled, created_at, updated_at, created_by",
            bind_count
        ));

        let mut q = sqlx::query_as::<sqlx::Postgres, PlanRow>(&query);

        if let Some(name) = plan_name {
            q = q.bind(name);
        }
        if let Some(balance) = initial_balance {
            q = q.bind(balance);
        }
        if let Some(limit) = credit_limit {
            q = q.bind(limit);
        }
        if let Some(calls) = max_concurrent_calls {
            q = q.bind(calls);
        }
        if let Some(desc) = description {
            q = q.bind(desc);
        }
        if let Some(en) = enabled {
            q = q.bind(en);
        }
        q = q.bind(id);

        let row = q.fetch_one(&self.pool).await.map_err(|e| {
            error!("Database error updating plan {}: {}", id, e);
            AppError::Database(format!("Failed to update plan: {}", e))
        })?;

        Ok(row.into())
    }

    /// Delete a plan
    #[instrument(skip(self))]
    pub async fn delete(&self, id: i32) -> AppResult<()> {
        debug!("Deleting plan: {}", id);

        let result = sqlx::query("DELETE FROM plans WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error deleting plan {}: {}", id, e);
                AppError::Database(format!("Failed to delete plan: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Plan {} not found", id)));
        }

        Ok(())
    }

    /// Count total plans
    #[instrument(skip(self))]
    pub async fn count(&self) -> AppResult<i64> {
        let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM plans")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error counting plans: {}", e);
                AppError::Database(format!("Failed to count plans: {}", e))
            })?;

        Ok(result.0)
    }
}
