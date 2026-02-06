//! Rate card repository implementation
//!
//! Provides PostgreSQL-backed storage for rate cards with optimized
//! Longest Prefix Match (LPM) algorithm for destination lookups.

use apolo_core::{
    models::RateCard,
    traits::{RateRepository, Repository},
    AppError, AppResult,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::{debug, error, instrument, warn};

/// PostgreSQL implementation of RateRepository
pub struct PgRateRepository {
    pool: PgPool,
}

impl PgRateRepository {
    /// Create a new rate repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Repository<RateCard, i32> for PgRateRepository {
    #[instrument(skip(self))]
    async fn find_by_id(&self, id: i32) -> AppResult<Option<RateCard>> {
        debug!("Finding rate card by id: {}", id);

        let result = sqlx::query_as::<sqlx::Postgres, RateCardRow>(
            r#"
            SELECT
                id, rate_name, destination_prefix, destination_name,
                rate_per_minute, billing_increment, connection_fee,
                effective_start, effective_end, priority,
                created_at, updated_at
            FROM rate_cards
            WHERE (effective_end IS NULL OR effective_end > NOW())
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding rate card {}: {}", id, e);
            AppError::Database(format!("Failed to find rate card: {}", e))
        })?;

        Ok(result.map(Into::into))
    }

    #[instrument(skip(self))]
    async fn find_all(&self, limit: i64, offset: i64) -> AppResult<Vec<RateCard>> {
        debug!(
            "Finding all rate cards with limit {} offset {}",
            limit, offset
        );

        let rows = sqlx::query_as::<sqlx::Postgres, RateCardRow>(
            r#"
            SELECT
                id, rate_name, destination_prefix, destination_name,
                rate_per_minute, billing_increment, connection_fee,
                effective_start, effective_end, priority,
                created_at, updated_at
            FROM rate_cards
            WHERE (effective_end IS NULL OR effective_end > NOW())
            ORDER BY priority DESC, destination_prefix
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error finding rate cards: {}", e);
            AppError::Database(format!("Failed to fetch rate cards: {}", e))
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    #[instrument(skip(self))]
    async fn count(&self) -> AppResult<i64> {
        let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM rate_cards WHERE (effective_end IS NULL OR effective_end > NOW())")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error counting rate cards: {}", e);
                AppError::Database(format!("Failed to count rate cards: {}", e))
            })?;

        Ok(result.0)
    }

    #[instrument(skip(self, entity))]
    async fn create(&self, entity: &RateCard) -> AppResult<RateCard> {
        debug!(
            "Creating rate card for prefix: {}",
            entity.destination_prefix
        );

        let rate_name = entity
            .rate_name
            .clone()
            .unwrap_or_else(|| format!("Rate {}", entity.destination_prefix));

        let row = sqlx::query_as::<sqlx::Postgres, RateCardRow>(
            r#"
            INSERT INTO rate_cards (
                rate_name, destination_prefix, destination_name,
                rate_per_minute, billing_increment, connection_fee,
                effective_start, effective_end, priority
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING
                id, rate_name, destination_prefix, destination_name,
                rate_per_minute, billing_increment, connection_fee,
                effective_start, effective_end, priority,
                created_at, updated_at
            "#,
        )
        .bind(&rate_name)
        .bind(&entity.destination_prefix)
        .bind(&entity.destination_name)
        .bind(entity.rate_per_minute)
        .bind(entity.billing_increment)
        .bind(entity.connection_fee)
        .bind(entity.effective_start)
        .bind(entity.effective_end)
        .bind(entity.priority)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error creating rate card: {}", e);
            AppError::Database(format!("Failed to create rate card: {}", e))
        })?;

        Ok(row.into())
    }

    #[instrument(skip(self, entity))]
    async fn update(&self, entity: &RateCard) -> AppResult<RateCard> {
        debug!("Updating rate card: {}", entity.id);

        let rate_name = entity
            .rate_name
            .clone()
            .unwrap_or_else(|| format!("Rate {}", entity.destination_prefix));

        let row = sqlx::query_as::<sqlx::Postgres, RateCardRow>(
            r#"
            UPDATE rate_cards
            SET rate_name = $2,
                destination_prefix = $3,
                destination_name = $4,
                rate_per_minute = $5,
                billing_increment = $6,
                connection_fee = $7,
                effective_start = $8,
                effective_end = $9,
                priority = $10,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id, rate_name, destination_prefix, destination_name,
                rate_per_minute, billing_increment, connection_fee,
                effective_start, effective_end, priority,
                created_at, updated_at
            "#,
        )
        .bind(entity.id)
        .bind(&rate_name)
        .bind(&entity.destination_prefix)
        .bind(&entity.destination_name)
        .bind(entity.rate_per_minute)
        .bind(entity.billing_increment)
        .bind(entity.connection_fee)
        .bind(entity.effective_start)
        .bind(entity.effective_end)
        .bind(entity.priority)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!("Database error updating rate card {}: {}", entity.id, e);
            AppError::Database(format!("Failed to update rate card: {}", e))
        })?;

        Ok(row.into())
    }

    #[instrument(skip(self))]
    async fn delete(&self, id: i32) -> AppResult<bool> {
        debug!("Deleting rate card: {}", id);

        let result = sqlx::query("DELETE FROM rate_cards WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error deleting rate card {}: {}", id, e);
                AppError::Database(format!("Failed to delete rate card: {}", e))
            })?;

        Ok(result.rows_affected() > 0)
    }
}

#[async_trait]
impl RateRepository for PgRateRepository {
    #[instrument(skip(self))]
    async fn find_by_destination(&self, destination: &str) -> AppResult<Option<RateCard>> {
        debug!("Finding rate for destination: {}", destination);

        // Normalize the destination (remove non-digits)
        let normalized = RateCard::normalize_destination(destination);

        // Generate all possible prefixes from longest to shortest
        let prefixes = RateCard::generate_prefixes(&normalized);

        if prefixes.is_empty() {
            warn!("No prefixes generated for destination: {}", destination);
            return Ok(None);
        }

        debug!("Generated {} prefixes for LPM lookup", prefixes.len());

        // Use PostgreSQL's ANY() for efficient prefix matching
        // The query will return the longest matching prefix with highest priority
        let result = sqlx::query_as::<sqlx::Postgres, RateCardRow>(
            r#"
            SELECT
                id, rate_name, destination_prefix, destination_name,
                rate_per_minute, billing_increment, connection_fee,
                effective_start, effective_end, priority,
                created_at, updated_at
            FROM rate_cards
            WHERE (effective_end IS NULL OR effective_end > NOW())
            WHERE destination_prefix = ANY($1)
                AND effective_start <= NOW()
                AND (effective_end IS NULL OR effective_end > NOW())
            ORDER BY
                LENGTH(destination_prefix) DESC,
                priority DESC
            LIMIT 1
            "#,
        )
        .bind(&prefixes)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            error!(
                "Database error finding rate for destination {}: {}",
                destination, e
            );
            AppError::Database(format!("Failed to find rate: {}", e))
        })?;

        if result.is_none() {
            debug!("No rate found for destination: {}", destination);
        }

        Ok(result.map(Into::into))
    }

    #[instrument(skip(self))]
    async fn search(
        &self,
        prefix: Option<&str>,
        name: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> AppResult<(Vec<RateCard>, i64)> {
        debug!(
            "Searching rates with prefix={:?}, name={:?}, limit={}, offset={}",
            prefix, name, limit, offset
        );

        // Build dynamic query based on search parameters
        let mut query_str = String::from(
            r#"
            SELECT
                id, rate_name, destination_prefix, destination_name,
                rate_per_minute, billing_increment, connection_fee,
                effective_start, effective_end, priority,
                created_at, updated_at
            FROM rate_cards
            WHERE (effective_end IS NULL OR effective_end > NOW())
            WHERE 1=1
            "#,
        );

        let mut count_query = String::from("SELECT COUNT(*) FROM rate_cards WHERE (effective_end IS NULL OR effective_end > NOW()) WHERE 1=1");

        if let Some(p) = prefix {
            let pattern = format!(" AND destination_prefix LIKE '{}%'", p.replace('\'', "''"));
            query_str.push_str(&pattern);
            count_query.push_str(&pattern);
        }

        if let Some(n) = name {
            let pattern = format!(" AND destination_name ILIKE '%{}%'", n.replace('\'', "''"));
            query_str.push_str(&pattern);
            count_query.push_str(&pattern);
        }

        query_str.push_str(&format!(
            " ORDER BY priority DESC, destination_prefix LIMIT {} OFFSET {}",
            limit, offset
        ));

        // Get total count
        let total: (i64,) = sqlx::query_as(&count_query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error counting searched rates: {}", e);
                AppError::Database(format!("Failed to count rates: {}", e))
            })?;

        // Get rate cards
        let rows = sqlx::query_as::<sqlx::Postgres, RateCardRow>(&query_str)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                error!("Database error searching rates: {}", e);
                AppError::Database(format!("Failed to search rates: {}", e))
            })?;

        Ok((rows.into_iter().map(Into::into).collect(), total.0))
    }

    #[instrument(skip(self, rates))]
    async fn bulk_create(&self, rates: &[RateCard]) -> AppResult<usize> {
        debug!("Bulk creating {} rate cards", rates.len());

        if rates.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await.map_err(|e| {
            error!("Failed to start transaction: {}", e);
            AppError::Transaction(format!("Failed to start transaction: {}", e))
        })?;

        let mut inserted = 0;

        for rate in rates {
            let rate_name = rate
                .rate_name
                .clone()
                .unwrap_or_else(|| format!("Rate {}", rate.destination_prefix));

            let result = sqlx::query(
                r#"
                INSERT INTO rate_cards (
                    rate_name, destination_prefix, destination_name,
                    rate_per_minute, billing_increment, connection_fee,
                    effective_start, effective_end, priority
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
            )
            .bind(&rate_name)
            .bind(&rate.destination_prefix)
            .bind(&rate.destination_name)
            .bind(rate.rate_per_minute)
            .bind(rate.billing_increment)
            .bind(rate.connection_fee)
            .bind(rate.effective_start)
            .bind(rate.effective_end)
            .bind(rate.priority)
            .execute(&mut *tx)
            .await;

            match result {
                Ok(_) => inserted += 1,
                Err(e) => {
                    warn!("Failed to insert rate {}: {}", rate.destination_prefix, e);
                    // Continue with other rates
                }
            }
        }

        tx.commit().await.map_err(|e| {
            error!("Failed to commit transaction: {}", e);
            AppError::Transaction(format!("Failed to commit transaction: {}", e))
        })?;

        debug!("Successfully inserted {} rate cards", inserted);
        Ok(inserted)
    }
}

/// Helper struct for mapping database rows
#[derive(Debug, sqlx::FromRow)]
struct RateCardRow {
    id: i32,
    rate_name: Option<String>,
    destination_prefix: String,
    destination_name: String,
    rate_per_minute: Decimal,
    billing_increment: i32,
    connection_fee: Decimal,
    effective_start: DateTime<Utc>,
    effective_end: Option<DateTime<Utc>>,
    priority: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<RateCardRow> for RateCard {
    fn from(row: RateCardRow) -> Self {
        Self {
            id: row.id,
            rate_name: row.rate_name,
            destination_prefix: row.destination_prefix,
            destination_name: row.destination_name,
            rate_per_minute: row.rate_per_minute,
            billing_increment: row.billing_increment,
            connection_fee: row.connection_fee,
            effective_start: row.effective_start,
            effective_end: row.effective_end,
            priority: row.priority,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_prefixes() {
        let prefixes = RateCard::generate_prefixes("51999");
        assert_eq!(prefixes, vec!["51999", "5199", "519", "51", "5"]);
    }

    #[test]
    fn test_normalize_destination() {
        assert_eq!(
            RateCard::normalize_destination("+51-999-888-777"),
            "51999888777"
        );
        assert_eq!(
            RateCard::normalize_destination("1 (555) 123-4567"),
            "15551234567"
        );
    }
}
