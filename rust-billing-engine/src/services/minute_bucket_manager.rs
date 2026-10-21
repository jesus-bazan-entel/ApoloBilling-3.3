// src/services/minute_bucket_manager.rs
//
// Minute-bucket billing engine (Plan Control / Plan Abierto) — the only
// billing model every account uses.
//
// Cycle = full calendar month, identical for every account, opened lazily on
// first call attempt of the month (with optional proration on an account's
// very first cycle ever). Concurrency safety: `SELECT ... FOR UPDATE` inside
// a transaction for the check-and-reserve and consume paths, `ON CONFLICT DO
// NOTHING` for the lazy cycle-open race.

use crate::cache::RedisClient;
use crate::database::DbPool;
use crate::error::BillingError;
use crate::models::Account;
use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Minutes soft-held per in-progress call, mirrors INITIAL_RESERVATION_MINUTES
/// in reservation_manager.rs. Prevents concurrent calls on a Control account
/// from all reading the same "disponible" before any of them commits.
const RESERVE_MINUTES: i32 = 5;

/// TTL for the Redis marker linking a call_uuid to its bucket cycle, matches
/// RESERVATION_TTL in reservation_manager.rs.
const BUCKET_HOLD_TTL: usize = 2700;

/// Default billing increment (seconds) used when tariffing overage with a
/// flat rate (no rate card to read an increment from).
const DEFAULT_OVERAGE_INCREMENT: i32 = 6;

const CYCLE_COLS: &str = "id, account_id, cycle_start, cycle_end, included_minutes, \
    is_prorated, proration_days, additional_minutes, reserved_minutes, consumed_minutes, \
    overage_minutes, overage_cost, status";

#[derive(Debug, Clone)]
pub struct CycleRow {
    pub id: i32,
    pub account_id: i32,
    #[allow(dead_code)]
    pub cycle_start: NaiveDate,
    #[allow(dead_code)]
    pub cycle_end: NaiveDate,
    pub included_minutes: i32,
    #[allow(dead_code)]
    pub is_prorated: bool,
    #[allow(dead_code)]
    pub proration_days: Option<i32>,
    pub additional_minutes: i32,
    pub reserved_minutes: Decimal,
    pub consumed_minutes: Decimal,
    #[allow(dead_code)]
    pub overage_minutes: Decimal,
    #[allow(dead_code)]
    pub overage_cost: Decimal,
    #[allow(dead_code)]
    pub status: String,
}

fn row_to_cycle(row: &tokio_postgres::Row) -> CycleRow {
    CycleRow {
        id: row.get(0),
        account_id: row.get(1),
        cycle_start: row.get(2),
        cycle_end: row.get(3),
        included_minutes: row.get(4),
        is_prorated: row.get(5),
        proration_days: row.get(6),
        additional_minutes: row.get(7),
        reserved_minutes: row.get(8),
        consumed_minutes: row.get(9),
        overage_minutes: row.get(10),
        overage_cost: row.get(11),
        status: row.get(12),
    }
}

/// Result of an authorization attempt for a minute-bucket account.
pub enum BucketAuthResult {
    /// Control account with no minutes left for a NEW call.
    Denied { reason: String },
    /// Call authorized; `cycle_id` is the bucket cycle it was charged against.
    Authorized { cycle_id: i32 },
}

/// Result of consuming a finished call's minutes against its bucket cycle.
pub struct BucketConsumeResult {
    pub account_id: i32,
    pub bucket_cycle_id: i32,
    pub in_bucket_minutes: Decimal,
    pub overage_minutes: Decimal,
    pub overage_cost: Decimal,
    pub overage_rate_applied: Option<Decimal>,
}

/// Outcome of `consume()` — distinguishes "this call wasn't a bucket call"
/// (no Redis hold found; caller should fall back to the balance/CDR path
/// unchanged) from an actual bucket consumption result.
pub enum ConsumeOutcome {
    NotApplicable,
    Consumed(BucketConsumeResult),
}

#[derive(Serialize, Deserialize)]
struct BucketHold {
    account_id: i32,
    cycle_id: i32,
}

pub struct MinuteBucketManager {
    db_pool: DbPool,
    redis: RedisClient,
}

impl MinuteBucketManager {
    pub fn new(db_pool: DbPool, redis: RedisClient) -> Self {
        Self { db_pool, redis }
    }

    /// First and last day of the calendar month containing `now`.
    fn month_bounds(now: chrono::DateTime<Utc>) -> (NaiveDate, NaiveDate) {
        let d = now.date_naive();
        let start = NaiveDate::from_ymd_opt(d.year(), d.month(), 1).expect("valid date");
        let (next_y, next_m) = if d.month() == 12 {
            (d.year() + 1, 1)
        } else {
            (d.year(), d.month() + 1)
        };
        let end = NaiveDate::from_ymd_opt(next_y, next_m, 1)
            .expect("valid date")
            .pred_opt()
            .expect("valid date");
        (start, end)
    }

    /// Lazily open (or return the existing) cycle for the current calendar
    /// month. Idempotent/concurrency-safe via
    /// `ON CONFLICT (account_id, cycle_start) DO NOTHING`.
    pub async fn ensure_open_cycle(&self, account_id: i32) -> Result<CycleRow, BillingError> {
        let (cycle_start, cycle_end) = Self::month_bounds(Utc::now());

        let client = self
            .db_pool
            .get()
            .await
            .map_err(|e| BillingError::Internal(e.to_string()))?;

        let select_sql = format!(
            "SELECT {cols} FROM minute_bucket_cycles WHERE account_id = $1 AND cycle_start = $2",
            cols = CYCLE_COLS
        );

        if let Some(row) = client
            .query_opt(&select_sql, &[&account_id, &cycle_start])
            .await?
        {
            return Ok(row_to_cycle(&row));
        }

        let insert_sql = format!(
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
                $1, $2::date, $3::date,
                CASE WHEN should_prorate
                    THEN GREATEST(0, ROUND(base_minutes::numeric * ($3::date - effective_start + 1)::numeric / days_in_month::numeric))::int
                    ELSE base_minutes END,
                should_prorate,
                CASE WHEN should_prorate THEN ($3::date - effective_start + 1) ELSE NULL END
            FROM calc
            ON CONFLICT (account_id, cycle_start) DO NOTHING
            RETURNING {cols}
            "#,
            cols = CYCLE_COLS
        );

        if let Some(row) = client
            .query_opt(&insert_sql, &[&account_id, &cycle_start, &cycle_end])
            .await?
        {
            return Ok(row_to_cycle(&row));
        }

        // Lost the race to a concurrent request; read what the winner inserted.
        let row = client
            .query_opt(&select_sql, &[&account_id, &cycle_start])
            .await?
            .ok_or_else(|| {
                BillingError::Internal("Failed to open or read bucket cycle".to_string())
            })?;
        Ok(row_to_cycle(&row))
    }

    /// Check-and-reserve for a new call attempt. Denies Control accounts with
    /// no minutes left; never denies Abierto. On success, soft-holds
    /// RESERVE_MINUTES on the cycle and remembers the (account, cycle) via a
    /// Redis marker keyed by call_uuid so `consume()` can find it later.
    pub async fn check_and_reserve(
        &self,
        account: &Account,
        call_uuid: &str,
    ) -> Result<BucketAuthResult, BillingError> {
        // Cheap idempotent open outside the row lock.
        self.ensure_open_cycle(account.id).await?;

        let mut client = self
            .db_pool
            .get()
            .await
            .map_err(|e| BillingError::Internal(e.to_string()))?;
        let tx = client.transaction().await?;

        let (cycle_start, _) = Self::month_bounds(Utc::now());
        let select_sql = format!(
            "SELECT {cols} FROM minute_bucket_cycles WHERE account_id = $1 AND cycle_start = $2 FOR UPDATE",
            cols = CYCLE_COLS
        );
        let row = tx.query_one(&select_sql, &[&account.id, &cycle_start]).await?;
        let cycle = row_to_cycle(&row);

        let disponible = Decimal::from(cycle.included_minutes + cycle.additional_minutes)
            - cycle.consumed_minutes
            - cycle.reserved_minutes;

        // bucket_type is nullable in the DB; treat missing/unknown as
        // "abierto" (never block) rather than silently blocking traffic.
        let is_control = account.bucket_type.as_deref() == Some("control");

        if is_control && disponible <= Decimal::ZERO {
            tx.rollback().await.ok();
            warn!(
                "🚫 Bucket EXHAUSTED (Control): account {} disponible={}",
                account.account_number, disponible
            );
            return Ok(BucketAuthResult::Denied {
                reason: "bucket_exhausted".to_string(),
            });
        }

        tx.execute(
            "UPDATE minute_bucket_cycles SET reserved_minutes = reserved_minutes + $2, updated_at = NOW() WHERE id = $1",
            &[&cycle.id, &Decimal::from(RESERVE_MINUTES)],
        )
        .await?;

        tx.commit().await?;

        let hold = BucketHold {
            account_id: account.id,
            cycle_id: cycle.id,
        };
        if let Ok(json) = serde_json::to_string(&hold) {
            let _ = self
                .redis
                .set(&format!("bucket_hold:{}", call_uuid), &json, BUCKET_HOLD_TTL)
                .await;
        }

        info!(
            "✅ Bucket call authorized: account {} cycle {} (disponible antes: {})",
            account.account_number, cycle.id, disponible
        );

        Ok(BucketAuthResult::Authorized { cycle_id: cycle.id })
    }

    /// Consume a finished call's billed minutes against its bucket cycle.
    /// Returns `ConsumeOutcome::NotApplicable` if this call was never
    /// authorized under the bucket model (no Redis hold) — callers should
    /// fall back to their existing (dollar/CDR-only) logic in that case.
    /// Looks the account's overage config up itself (by the id stored in the
    /// hold) so `CdrGenerator` doesn't need to know in advance whether a call
    /// was a bucket call before calling this.
    pub async fn consume(
        &self,
        call_uuid: &str,
        billed_minutes: Decimal,
        destination: &str,
    ) -> Result<ConsumeOutcome, BillingError> {
        let hold_key = format!("bucket_hold:{}", call_uuid);
        let hold_json = match self.redis.get(&hold_key).await {
            Ok(Some(v)) => v,
            _ => return Ok(ConsumeOutcome::NotApplicable),
        };
        let hold: BucketHold = match serde_json::from_str(&hold_json) {
            Ok(h) => h,
            Err(_) => return Ok(ConsumeOutcome::NotApplicable),
        };

        let lookup_client = self
            .db_pool
            .get()
            .await
            .map_err(|e| BillingError::Internal(e.to_string()))?;
        let acc_row = lookup_client
            .query_opt(
                "SELECT account_number, overage_flat_rate_per_minute, overage_tariff_group FROM accounts WHERE id = $1",
                &[&hold.account_id],
            )
            .await?;
        let (account_number, overage_flat_rate, overage_group): (String, Option<Decimal>, Option<String>) =
            match acc_row {
                Some(r) => (r.get(0), r.get(1), r.get(2)),
                None => return Ok(ConsumeOutcome::NotApplicable),
            };
        drop(lookup_client);

        let mut client = self
            .db_pool
            .get()
            .await
            .map_err(|e| BillingError::Internal(e.to_string()))?;
        let tx = client.transaction().await?;

        let select_sql = format!(
            "SELECT {cols} FROM minute_bucket_cycles WHERE id = $1 FOR UPDATE",
            cols = CYCLE_COLS
        );
        let row = tx.query_one(&select_sql, &[&hold.cycle_id]).await?;
        let cycle = row_to_cycle(&row);

        let disponible_antes = (Decimal::from(cycle.included_minutes + cycle.additional_minutes)
            - cycle.consumed_minutes)
            .max(Decimal::ZERO);
        let in_bucket = billed_minutes.min(disponible_antes);
        let overage = billed_minutes - in_bucket;

        let (overage_cost, overage_rate_applied) = if overage > Decimal::ZERO {
            match self
                .resolve_overage_rate(overage_flat_rate, overage_group.as_deref(), destination)
                .await?
            {
                Some((rate, _increment)) => (overage * rate, Some(rate)),
                None => (Decimal::ZERO, None),
            }
        } else {
            (Decimal::ZERO, None)
        };

        tx.execute(
            "UPDATE minute_bucket_cycles
             SET consumed_minutes = consumed_minutes + $2,
                 overage_minutes = overage_minutes + $3,
                 overage_cost = overage_cost + $4,
                 reserved_minutes = GREATEST(0, reserved_minutes - $5),
                 updated_at = NOW()
             WHERE id = $1",
            &[
                &cycle.id,
                &in_bucket,
                &overage,
                &overage_cost,
                &Decimal::from(RESERVE_MINUTES),
            ],
        )
        .await?;

        tx.commit().await?;

        let _ = self.redis.delete(&hold_key).await;

        // Best-effort 80%/100% alert enqueue — never fails the call/CDR path.
        let new_consumed = cycle.consumed_minutes + in_bucket;
        let total = Decimal::from(cycle.included_minutes + cycle.additional_minutes);
        if total > Decimal::ZERO {
            let utilization = (new_consumed / total) * Decimal::from(100);
            for threshold in [100i16, 80i16] {
                if utilization >= Decimal::from(threshold) {
                    if let Err(e) = self
                        .maybe_enqueue_alert(hold.account_id, cycle.id, threshold)
                        .await
                    {
                        warn!("Failed to enqueue bucket alert: {}", e);
                    }
                }
            }
        }

        info!(
            "💰 Bucket consumed: account {} cycle {} in_bucket={} overage={} overage_cost=${}",
            account_number, cycle.id, in_bucket, overage, overage_cost
        );

        Ok(ConsumeOutcome::Consumed(BucketConsumeResult {
            account_id: hold.account_id,
            bucket_cycle_id: cycle.id,
            in_bucket_minutes: in_bucket,
            overage_minutes: overage,
            overage_cost,
            overage_rate_applied,
        }))
    }

    async fn maybe_enqueue_alert(
        &self,
        account_id: i32,
        cycle_id: i32,
        threshold_pct: i16,
    ) -> Result<(), BillingError> {
        let client = self
            .db_pool
            .get()
            .await
            .map_err(|e| BillingError::Internal(e.to_string()))?;
        client
            .execute(
                "INSERT INTO minute_bucket_alerts (account_id, cycle_id, threshold_pct, status)
                 SELECT $1, $2, $3, 'pending'
                 WHERE EXISTS (SELECT 1 FROM accounts WHERE id = $1 AND notification_email IS NOT NULL)
                 ON CONFLICT (cycle_id, threshold_pct) DO NOTHING",
                &[&account_id, &cycle_id, &threshold_pct],
            )
            .await?;
        Ok(())
    }

    /// Resolves the per-minute overage rate: flat rate first, else LPM
    /// against `rate_cards` scoped to `overage_group` (or `'standard'` if
    /// unset — i.e. the same rates normal calls use).
    async fn resolve_overage_rate(
        &self,
        flat_rate: Option<Decimal>,
        overage_group: Option<&str>,
        destination: &str,
    ) -> Result<Option<(Decimal, i32)>, BillingError> {
        if let Some(flat) = flat_rate {
            return Ok(Some((flat, DEFAULT_OVERAGE_INCREMENT)));
        }

        let group = overage_group.unwrap_or("standard").to_string();

        let normalized = destination.replace('+', "");
        let mut prefixes: Vec<String> = Vec::new();
        for i in (1..=normalized.len()).rev() {
            prefixes.push(normalized[..i].to_string());
        }
        if prefixes.is_empty() {
            return Ok(None);
        }

        let client = self
            .db_pool
            .get()
            .await
            .map_err(|e| BillingError::Internal(e.to_string()))?;
        let row = client
            .query_opt(
                "SELECT rate_per_minute, billing_increment
                 FROM rate_cards
                 WHERE destination_prefix = ANY($1)
                 AND card_group = $2
                 AND effective_start <= NOW()
                 AND (effective_end IS NULL OR effective_end >= NOW())
                 ORDER BY LENGTH(destination_prefix) DESC, priority DESC
                 LIMIT 1",
                &[&prefixes, &group],
            )
            .await?;

        Ok(row.map(|r| {
            let rate: Decimal = r.get(0);
            let increment: i32 = r.get(1);
            (rate, increment)
        }))
    }
}
