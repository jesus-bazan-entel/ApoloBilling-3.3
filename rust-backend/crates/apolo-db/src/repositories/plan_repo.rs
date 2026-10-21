//! Plan repository implementation
//!
//! Provides PostgreSQL-backed storage for plan entities (minute-bucket
//! templates: Plan Control / Plan Abierto).

use apolo_core::{models::Plan, AppError, AppResult};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, PgPool};
use tracing::{debug, error, instrument};

const PLAN_COLS: &str = "id, plan_name, plan_code, bucket_type, included_minutes, \
    proration_mode, overage_flat_rate_per_minute, overage_tariff_group, max_concurrent_calls, \
    description, enabled, created_at, updated_at, created_by";

/// Database row representation of a plan
#[derive(Debug, FromRow)]
struct PlanRow {
    id: i32,
    plan_name: String,
    plan_code: String,
    bucket_type: String,
    included_minutes: i32,
    proration_mode: String,
    overage_flat_rate_per_minute: Option<Decimal>,
    overage_tariff_group: Option<String>,
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
            bucket_type: row.bucket_type,
            included_minutes: row.included_minutes,
            proration_mode: row.proration_mode,
            overage_flat_rate_per_minute: row.overage_flat_rate_per_minute,
            overage_tariff_group: row.overage_tariff_group,
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

        let result = sqlx::query_as::<sqlx::Postgres, PlanRow>(&format!(
            "SELECT {PLAN_COLS} FROM plans WHERE id = $1"
        ))
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

        let result = sqlx::query_as::<sqlx::Postgres, PlanRow>(&format!(
            "SELECT {PLAN_COLS} FROM plans WHERE plan_code = $1"
        ))
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

        let rows = sqlx::query_as::<sqlx::Postgres, PlanRow>(&format!(
            "SELECT {PLAN_COLS} FROM plans ORDER BY bucket_type, plan_name"
        ))
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

        let rows = sqlx::query_as::<sqlx::Postgres, PlanRow>(&format!(
            "SELECT {PLAN_COLS} FROM plans WHERE enabled = true ORDER BY bucket_type, plan_name"
        ))
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
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        plan_name: &str,
        plan_code: &str,
        bucket_type: &str,
        included_minutes: i32,
        proration_mode: &str,
        overage_flat_rate_per_minute: Option<Decimal>,
        overage_tariff_group: Option<&str>,
        max_concurrent_calls: i32,
        description: Option<&str>,
        enabled: bool,
        created_by: &str,
    ) -> AppResult<Plan> {
        debug!("Creating plan: {}", plan_code);

        let row = sqlx::query_as::<sqlx::Postgres, PlanRow>(&format!(
            r#"
            INSERT INTO plans (
                plan_name, plan_code, bucket_type, included_minutes, proration_mode,
                overage_flat_rate_per_minute, overage_tariff_group, max_concurrent_calls,
                description, enabled, created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING {PLAN_COLS}
            "#
        ))
        .bind(plan_name)
        .bind(plan_code)
        .bind(bucket_type)
        .bind(included_minutes)
        .bind(proration_mode)
        .bind(overage_flat_rate_per_minute)
        .bind(overage_tariff_group)
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

    /// Update an existing plan's general template fields. Bucket
    /// configuration (bucket_type, included_minutes, proration_mode,
    /// overage settings) is updated via `PgMinuteBucketRepository` instead.
    #[instrument(skip(self))]
    pub async fn update(
        &self,
        id: i32,
        plan_name: Option<&str>,
        max_concurrent_calls: Option<i32>,
        description: Option<&str>,
        enabled: Option<bool>,
    ) -> AppResult<Plan> {
        debug!("Updating plan: {}", id);

        let mut query = String::from("UPDATE plans SET updated_at = NOW()");
        let mut bind_count = 1;

        if plan_name.is_some() {
            query.push_str(&format!(", plan_name = ${}", bind_count));
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

        query.push_str(&format!(" WHERE id = ${bind_count} RETURNING {PLAN_COLS}"));

        let mut q = sqlx::query_as::<sqlx::Postgres, PlanRow>(&query);

        if let Some(name) = plan_name {
            q = q.bind(name);
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
