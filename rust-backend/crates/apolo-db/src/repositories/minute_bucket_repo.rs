//! Minute bucket repository implementation
//!
//! PostgreSQL-backed storage for the "bolsa de minutos" billing model:
//! plan/account bucket configuration, monthly cycles (lazy-opened with
//! optional proration), manual top-ups, the monthly consumption report,
//! and the 80%/100% email alert queue.

use apolo_core::{
    models::{
        AccountBucketConfig, BillingModel, BucketConfig, BucketType, MinuteBucketAddition,
        MinuteBucketAlert, MinuteBucketCycle, MinuteBucketReportRow, PlanBucketConfig,
        ProrationMode,
    },
    AppError, AppResult,
};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::{debug, error, instrument, warn};

/// PostgreSQL implementation of the minute-bucket repository.
pub struct PgMinuteBucketRepository {
    pool: PgPool,
}

/// First and last day of the calendar month containing `date`.
fn month_bounds(date: NaiveDate) -> (NaiveDate, NaiveDate) {
    let start = NaiveDate::from_ymd_opt(date.year(), date.month(), 1).expect("valid date");
    let (next_y, next_m) = if date.month() == 12 {
        (date.year() + 1, 1)
    } else {
        (date.year(), date.month() + 1)
    };
    let end = NaiveDate::from_ymd_opt(next_y, next_m, 1)
        .expect("valid date")
        .pred_opt()
        .expect("valid date");
    (start, end)
}

#[derive(Debug, sqlx::FromRow)]
struct CycleRow {
    id: i32,
    account_id: i32,
    cycle_start: NaiveDate,
    cycle_end: NaiveDate,
    included_minutes: i32,
    is_prorated: bool,
    proration_days: Option<i32>,
    additional_minutes: i32,
    reserved_minutes: Decimal,
    consumed_minutes: Decimal,
    overage_minutes: Decimal,
    overage_cost: Decimal,
    alert_80_sent_at: Option<DateTime<Utc>>,
    alert_100_sent_at: Option<DateTime<Utc>>,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<CycleRow> for MinuteBucketCycle {
    fn from(r: CycleRow) -> Self {
        Self {
            id: r.id,
            account_id: r.account_id,
            cycle_start: r.cycle_start,
            cycle_end: r.cycle_end,
            included_minutes: r.included_minutes,
            is_prorated: r.is_prorated,
            proration_days: r.proration_days,
            additional_minutes: r.additional_minutes,
            reserved_minutes: r.reserved_minutes,
            consumed_minutes: r.consumed_minutes,
            overage_minutes: r.overage_minutes,
            overage_cost: r.overage_cost,
            alert_80_sent_at: r.alert_80_sent_at,
            alert_100_sent_at: r.alert_100_sent_at,
            status: r.status,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

const CYCLE_COLUMNS: &str = r#"
    id, account_id, cycle_start, cycle_end, included_minutes, is_prorated,
    proration_days, additional_minutes, reserved_minutes, consumed_minutes,
    overage_minutes, overage_cost, alert_80_sent_at, alert_100_sent_at,
    status, created_at, updated_at
"#;

impl PgMinuteBucketRepository {
    /// Create a new repository instance.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ================================================================
    // Plan bucket config
    // ================================================================

    /// Read a plan's minute-bucket template.
    #[instrument(skip(self))]
    pub async fn get_plan_bucket_config(&self, plan_id: i32) -> AppResult<Option<PlanBucketConfig>> {
        let row = sqlx::query_as::<_, (i32, String, Option<String>, i32, String, Option<Decimal>, Option<String>)>(
            r#"
            SELECT id, billing_model, bucket_type, included_minutes, proration_mode,
                   overage_flat_rate_per_minute, overage_tariff_group
            FROM plans WHERE id = $1
            "#,
        )
        .bind(plan_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error reading plan bucket config {}: {}", plan_id, e);
            AppError::Database(format!("Failed to read plan bucket config: {}", e))
        })?;

        Ok(row.map(
            |(id, billing_model, bucket_type, included_minutes, proration_mode, flat_rate, group)| {
                PlanBucketConfig {
                    plan_id: id,
                    config: BucketConfig {
                        billing_model: BillingModel::from_str(&billing_model),
                        bucket_type: bucket_type.and_then(|s| BucketType::from_str(&s)),
                        included_minutes,
                        proration_mode: ProrationMode::from_str(&proration_mode),
                        overage_flat_rate_per_minute: flat_rate,
                        overage_tariff_group: group,
                    },
                }
            },
        ))
    }

    /// Update a plan's minute-bucket template.
    #[instrument(skip(self, cfg))]
    pub async fn update_plan_bucket_config(
        &self,
        plan_id: i32,
        cfg: &BucketConfig,
    ) -> AppResult<PlanBucketConfig> {
        let bucket_type = cfg.bucket_type.map(|b| b.to_string());

        sqlx::query(
            r#"
            UPDATE plans
            SET billing_model = $2,
                bucket_type = $3,
                included_minutes = $4,
                proration_mode = $5,
                overage_flat_rate_per_minute = $6,
                overage_tariff_group = $7,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(plan_id)
        .bind(cfg.billing_model.to_string())
        .bind(&bucket_type)
        .bind(cfg.included_minutes)
        .bind(cfg.proration_mode.to_string())
        .bind(cfg.overage_flat_rate_per_minute)
        .bind(&cfg.overage_tariff_group)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error updating plan bucket config {}: {}", plan_id, e);
            AppError::Database(format!("Failed to update plan bucket config: {}", e))
        })?;

        self.get_plan_bucket_config(plan_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Plan {} not found", plan_id)))
    }

    // ================================================================
    // Account bucket config
    // ================================================================

    /// Read an account's effective minute-bucket configuration.
    #[instrument(skip(self))]
    pub async fn get_account_bucket_config(
        &self,
        account_id: i32,
    ) -> AppResult<Option<AccountBucketConfig>> {
        let row = sqlx::query_as::<_, (i32, String, Option<String>, i32, String, Option<Decimal>, Option<String>, Option<String>)>(
            r#"
            SELECT id, billing_model, bucket_type, included_minutes, proration_mode,
                   overage_flat_rate_per_minute, overage_tariff_group, notification_email
            FROM accounts WHERE id = $1
            "#,
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error reading account bucket config {}: {}", account_id, e);
            AppError::Database(format!("Failed to read account bucket config: {}", e))
        })?;

        Ok(row.map(
            |(id, billing_model, bucket_type, included_minutes, proration_mode, flat_rate, group, email)| {
                AccountBucketConfig {
                    account_id: id,
                    config: BucketConfig {
                        billing_model: BillingModel::from_str(&billing_model),
                        bucket_type: bucket_type.and_then(|s| BucketType::from_str(&s)),
                        included_minutes,
                        proration_mode: ProrationMode::from_str(&proration_mode),
                        overage_flat_rate_per_minute: flat_rate,
                        overage_tariff_group: group,
                    },
                    notification_email: email,
                }
            },
        ))
    }

    /// Update an account's minute-bucket configuration (override of the plan template).
    #[instrument(skip(self, cfg))]
    pub async fn update_account_bucket_config(
        &self,
        account_id: i32,
        cfg: &BucketConfig,
        notification_email: Option<&str>,
    ) -> AppResult<AccountBucketConfig> {
        let bucket_type = cfg.bucket_type.map(|b| b.to_string());

        sqlx::query(
            r#"
            UPDATE accounts
            SET billing_model = $2,
                bucket_type = $3,
                included_minutes = $4,
                proration_mode = $5,
                overage_flat_rate_per_minute = $6,
                overage_tariff_group = $7,
                notification_email = COALESCE($8, notification_email),
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(account_id)
        .bind(cfg.billing_model.to_string())
        .bind(&bucket_type)
        .bind(cfg.included_minutes)
        .bind(cfg.proration_mode.to_string())
        .bind(cfg.overage_flat_rate_per_minute)
        .bind(&cfg.overage_tariff_group)
        .bind(notification_email)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error updating account bucket config {}: {}", account_id, e);
            AppError::Database(format!("Failed to update account bucket config: {}", e))
        })?;

        self.get_account_bucket_config(account_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Account {} not found", account_id)))
    }

    // ================================================================
    // Cycles
    // ================================================================

    /// Read the cycle covering `date` for an account, if it has been opened.
    #[instrument(skip(self))]
    pub async fn get_cycle_for_date(
        &self,
        account_id: i32,
        date: NaiveDate,
    ) -> AppResult<Option<MinuteBucketCycle>> {
        let (cycle_start, _) = month_bounds(date);

        let query = format!(
            "SELECT {cols} FROM minute_bucket_cycles WHERE account_id = $1 AND cycle_start = $2",
            cols = CYCLE_COLUMNS
        );

        let row = sqlx::query_as::<_, CycleRow>(&query)
            .bind(account_id)
            .bind(cycle_start)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error reading bucket cycle for account {}: {}", account_id, e);
                AppError::Database(format!("Failed to read bucket cycle: {}", e))
            })?;

        Ok(row.map(Into::into))
    }

    /// Lazily open (or return the existing) cycle for the current calendar
    /// month, applying proration only on an account's very first cycle ever
    /// when `proration_mode = 'prorated'` and the account was created after
    /// the 1st of the month. Idempotent/concurrency-safe via
    /// `ON CONFLICT (account_id, cycle_start) DO NOTHING`.
    #[instrument(skip(self))]
    pub async fn ensure_open_cycle(&self, account_id: i32) -> AppResult<MinuteBucketCycle> {
        let today = Utc::now().date_naive();
        let (cycle_start, cycle_end) = month_bounds(today);

        if let Some(existing) = self.get_cycle_for_date(account_id, today).await? {
            return Ok(existing);
        }

        let insert_query = format!(
            r#"
            WITH acc AS (
                SELECT included_minutes, proration_mode, created_at::date AS created_date
                FROM accounts WHERE id = $1
            ),
            has_history AS (
                SELECT EXISTS(SELECT 1 FROM minute_bucket_cycles WHERE account_id = $1) AS any_cycle
            ),
            calc AS (
                SELECT
                    acc.included_minutes AS base_minutes,
                    (NOT has_history.any_cycle
                        AND acc.proration_mode = 'prorated'
                        AND acc.created_date > $2::date) AS should_prorate,
                    GREATEST(acc.created_date, $2::date) AS effective_start,
                    ($3::date - $2::date + 1) AS days_in_month
                FROM acc, has_history
            )
            INSERT INTO minute_bucket_cycles
                (account_id, cycle_start, cycle_end, included_minutes, is_prorated, proration_days)
            SELECT
                $1,
                $2::date,
                $3::date,
                CASE WHEN should_prorate
                    THEN GREATEST(0, ROUND(base_minutes::numeric * ($3::date - effective_start + 1)::numeric / days_in_month::numeric))::int
                    ELSE base_minutes
                END,
                should_prorate,
                CASE WHEN should_prorate THEN ($3::date - effective_start + 1) ELSE NULL END
            FROM calc
            ON CONFLICT (account_id, cycle_start) DO NOTHING
            RETURNING {cols}
            "#,
            cols = CYCLE_COLUMNS
        );

        let row = sqlx::query_as::<_, CycleRow>(&insert_query)
            .bind(account_id)
            .bind(cycle_start)
            .bind(cycle_end)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error opening bucket cycle for account {}: {}", account_id, e);
                AppError::Database(format!("Failed to open bucket cycle: {}", e))
            })?;

        match row {
            Some(r) => Ok(r.into()),
            // Lost the race to a concurrent request; read what the winner inserted.
            None => self
                .get_cycle_for_date(account_id, today)
                .await?
                .ok_or_else(|| AppError::Internal("Failed to open or read bucket cycle".to_string())),
        }
    }

    /// Manually override the open cycle's `included_minutes` (e.g. to adjust
    /// a prorated suggestion by hand).
    #[instrument(skip(self))]
    pub async fn set_cycle_included_minutes(
        &self,
        account_id: i32,
        included_minutes: i32,
    ) -> AppResult<MinuteBucketCycle> {
        let cycle = self.ensure_open_cycle(account_id).await?;

        let query = format!(
            "UPDATE minute_bucket_cycles SET included_minutes = $2, updated_at = NOW() WHERE id = $1 RETURNING {cols}",
            cols = CYCLE_COLUMNS
        );

        let row = sqlx::query_as::<_, CycleRow>(&query)
            .bind(cycle.id)
            .bind(included_minutes)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error updating cycle {} included_minutes: {}", cycle.id, e);
                AppError::Database(format!("Failed to update cycle: {}", e))
            })?;

        Ok(row.into())
    }

    /// Cycle history for an account (most recent first).
    #[instrument(skip(self))]
    pub async fn history(&self, account_id: i32) -> AppResult<Vec<MinuteBucketCycle>> {
        let query = format!(
            "SELECT {cols} FROM minute_bucket_cycles WHERE account_id = $1 ORDER BY cycle_start DESC",
            cols = CYCLE_COLUMNS
        );

        let rows = sqlx::query_as::<_, CycleRow>(&query)
            .bind(account_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error reading bucket history for account {}: {}", account_id, e);
                AppError::Database(format!("Failed to read bucket history: {}", e))
            })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Additions registered for a given cycle (most recent first).
    #[instrument(skip(self))]
    pub async fn additions_for_cycle(&self, cycle_id: i32) -> AppResult<Vec<MinuteBucketAddition>> {
        let rows = sqlx::query_as::<_, (i32, i32, i32, i32, Option<String>, String, DateTime<Utc>)>(
            r#"
            SELECT id, account_id, cycle_id, minutes_added, reason, created_by, created_at
            FROM minute_bucket_additions
            WHERE cycle_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(cycle_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error reading additions for cycle {}: {}", cycle_id, e);
            AppError::Database(format!("Failed to read additions: {}", e))
        })?;

        Ok(rows
            .into_iter()
            .map(
                |(id, account_id, cycle_id, minutes_added, reason, created_by, created_at)| {
                    MinuteBucketAddition {
                        id,
                        account_id,
                        cycle_id,
                        minutes_added,
                        reason,
                        created_by,
                        created_at,
                    }
                },
            )
            .collect())
    }

    /// Register a manual bolsa top-up: ensures an open cycle exists, inserts
    /// the addition row, and bumps `additional_minutes` on the cycle. In
    /// Control plans this immediately unblocks new outbound calls, since the
    /// billing engine re-reads `additional_minutes` on the next authorization
    /// attempt.
    #[instrument(skip(self))]
    pub async fn add_topup(
        &self,
        account_id: i32,
        minutes_added: i32,
        reason: Option<&str>,
        created_by: &str,
    ) -> AppResult<MinuteBucketAddition> {
        let cycle = self.ensure_open_cycle(account_id).await?;

        let mut tx = self.pool.begin().await.map_err(|e| {
            error!("Failed to start transaction for topup: {}", e);
            AppError::Transaction(format!("Failed to start transaction: {}", e))
        })?;

        let row = sqlx::query_as::<_, (i32, i32, i32, i32, Option<String>, String, DateTime<Utc>)>(
            r#"
            INSERT INTO minute_bucket_additions (account_id, cycle_id, minutes_added, reason, created_by)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, account_id, cycle_id, minutes_added, reason, created_by, created_at
            "#,
        )
        .bind(account_id)
        .bind(cycle.id)
        .bind(minutes_added)
        .bind(reason)
        .bind(created_by)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            error!("Database error inserting topup for account {}: {}", account_id, e);
            AppError::Database(format!("Failed to insert topup: {}", e))
        })?;

        sqlx::query(
            "UPDATE minute_bucket_cycles SET additional_minutes = additional_minutes + $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(cycle.id)
        .bind(minutes_added)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("Database error updating cycle additional_minutes {}: {}", cycle.id, e);
            AppError::Database(format!("Failed to update cycle: {}", e))
        })?;

        tx.commit().await.map_err(|e| {
            error!("Failed to commit topup transaction: {}", e);
            AppError::Transaction(format!("Failed to commit transaction: {}", e))
        })?;

        let (id, account_id, cycle_id, minutes_added, reason, created_by, created_at) = row;
        Ok(MinuteBucketAddition {
            id,
            account_id,
            cycle_id,
            minutes_added,
            reason,
            created_by,
            created_at,
        })
    }

    // ================================================================
    // Monthly report
    // ================================================================

    /// Monthly consumption report. `month` is any date within the target
    /// calendar month (only year/month are used).
    #[instrument(skip(self))]
    pub async fn monthly_report(
        &self,
        month: NaiveDate,
        account_id: Option<i32>,
        bucket_type: Option<&str>,
    ) -> AppResult<Vec<MinuteBucketReportRow>> {
        let (cycle_start, _) = month_bounds(month);

        let rows = sqlx::query_as::<_, (
            i32, String, Option<String>, Option<String>, i32, bool, i32, i64,
            Decimal, Decimal, Decimal, Decimal, String, bool,
        )>(
            r#"
            SELECT
                a.id AS account_id,
                a.account_number,
                a.account_name,
                a.bucket_type,
                c.included_minutes,
                c.is_prorated,
                c.additional_minutes,
                COALESCE(add_count.cnt, 0) AS additions_count,
                c.consumed_minutes,
                c.overage_minutes,
                c.overage_cost,
                CASE WHEN (c.included_minutes + c.additional_minutes) > 0
                     THEN (c.consumed_minutes / (c.included_minutes + c.additional_minutes)::numeric * 100)
                     ELSE 0 END AS utilization_percent,
                c.status AS cycle_status,
                (a.bucket_type = 'control'
                    AND (c.included_minutes + c.additional_minutes - c.consumed_minutes - c.reserved_minutes) <= 0
                ) AS is_blocked
            FROM accounts a
            JOIN minute_bucket_cycles c ON c.account_id = a.id AND c.cycle_start = $1
            LEFT JOIN (
                SELECT cycle_id, COUNT(*) AS cnt FROM minute_bucket_additions GROUP BY cycle_id
            ) add_count ON add_count.cycle_id = c.id
            WHERE a.billing_model = 'minute_bucket'
                AND ($2::int IS NULL OR a.id = $2)
                AND ($3::text IS NULL OR a.bucket_type = $3)
            ORDER BY a.account_number
            "#,
        )
        .bind(cycle_start)
        .bind(account_id)
        .bind(bucket_type)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error building minute bucket report: {}", e);
            AppError::Database(format!("Failed to build report: {}", e))
        })?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    account_id,
                    account_number,
                    account_name,
                    bucket_type,
                    included_minutes,
                    is_prorated,
                    additional_minutes,
                    additions_count,
                    consumed_minutes,
                    overage_minutes,
                    overage_cost,
                    utilization_percent,
                    cycle_status,
                    is_blocked,
                )| MinuteBucketReportRow {
                    account_id,
                    account_number,
                    account_name,
                    bucket_type,
                    included_minutes,
                    is_prorated,
                    additional_minutes,
                    additions_count,
                    consumed_minutes,
                    overage_minutes,
                    overage_cost,
                    utilization_percent,
                    cycle_status,
                    is_blocked,
                },
            )
            .collect())
    }

    // ================================================================
    // Email alerts (80% / 100%)
    // ================================================================

    /// Pending alerts ready to send, together with the account's notification
    /// email and display info. Alerts with no `notification_email` on the
    /// account are skipped (never enqueued as failed) since there's nowhere
    /// to send them; the caller should just leave them pending or the
    /// billing engine should not enqueue them in the first place.
    #[instrument(skip(self))]
    pub async fn pending_alerts(&self) -> AppResult<Vec<(MinuteBucketAlert, String, Option<String>)>> {
        let rows = sqlx::query_as::<_, (
            i32, i32, i32, i16, String, Option<String>, DateTime<Utc>, Option<DateTime<Utc>>,
            String, Option<String>,
        )>(
            r#"
            SELECT
                al.id, al.account_id, al.cycle_id, al.threshold_pct, al.status,
                al.error_message, al.created_at, al.sent_at,
                a.account_number, a.notification_email
            FROM minute_bucket_alerts al
            JOIN accounts a ON a.id = al.account_id
            WHERE al.status = 'pending'
            ORDER BY al.created_at ASC
            LIMIT 200
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error reading pending bucket alerts: {}", e);
            AppError::Database(format!("Failed to read pending alerts: {}", e))
        })?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    account_id,
                    cycle_id,
                    threshold_pct,
                    status,
                    error_message,
                    created_at,
                    sent_at,
                    account_number,
                    notification_email,
                )| {
                    (
                        MinuteBucketAlert {
                            id,
                            account_id,
                            cycle_id,
                            threshold_pct,
                            status,
                            error_message,
                            created_at,
                            sent_at,
                        },
                        account_number,
                        notification_email,
                    )
                },
            )
            .collect())
    }

    /// Mark an alert as successfully sent.
    #[instrument(skip(self))]
    pub async fn mark_alert_sent(&self, alert_id: i32) -> AppResult<()> {
        sqlx::query("UPDATE minute_bucket_alerts SET status = 'sent', sent_at = NOW() WHERE id = $1")
            .bind(alert_id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error marking alert {} sent: {}", alert_id, e);
                AppError::Database(format!("Failed to mark alert sent: {}", e))
            })?;
        Ok(())
    }

    /// Mark an alert as failed with an error message (kept for troubleshooting;
    /// not retried automatically to avoid spamming a broken mailbox).
    #[instrument(skip(self))]
    pub async fn mark_alert_failed(&self, alert_id: i32, error: &str) -> AppResult<()> {
        warn!("Marking bucket alert {} as failed: {}", alert_id, error);
        sqlx::query("UPDATE minute_bucket_alerts SET status = 'failed', error_message = $2 WHERE id = $1")
            .bind(alert_id)
            .bind(error)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error marking alert {} failed: {}", alert_id, e);
                AppError::Database(format!("Failed to mark alert failed: {}", e))
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_month_bounds_mid_year() {
        let d = NaiveDate::from_ymd_opt(2026, 9, 15).unwrap();
        let (start, end) = month_bounds(d);
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 9, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 9, 30).unwrap());
    }

    #[test]
    fn test_month_bounds_december() {
        let d = NaiveDate::from_ymd_opt(2026, 12, 25).unwrap();
        let (start, end) = month_bounds(d);
        assert_eq!(start, NaiveDate::from_ymd_opt(2026, 12, 1).unwrap());
        assert_eq!(end, NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
    }

    #[test]
    fn test_month_bounds_february_leap_year() {
        let d = NaiveDate::from_ymd_opt(2028, 2, 10).unwrap();
        let (start, end) = month_bounds(d);
        assert_eq!(end, NaiveDate::from_ymd_opt(2028, 2, 29).unwrap());
    }
}
