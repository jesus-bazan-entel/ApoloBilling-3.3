//! Billing sync service
//!
//! Synchronizes rate_cards table from zones, prefixes, and rate_zones tables.
//! This service rebuilds the denormalized rate_cards table used for fast LPM lookups.

use apolo_core::{AppError, AppResult};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

/// Billing sync service
///
/// Responsible for synchronizing the rate_cards table from the normalized
/// zones/prefixes/rate_zones schema.
pub struct BillingSyncService {
    pool: Arc<PgPool>,
}

impl BillingSyncService {
    /// Create a new billing sync service
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// Synchronize rate cards from zones, prefixes, and rate_zones
    ///
    /// This operation:
    /// 1. Truncates the rate_cards table
    /// 2. Rebuilds it from zones/prefixes/rate_zones with a single INSERT...SELECT
    /// 3. Returns the count of synced rows
    ///
    /// # Returns
    ///
    /// The number of rate cards created
    ///
    /// # Errors
    ///
    /// Returns `AppError::Database` if the sync operation fails
    #[instrument(skip(self))]
    pub async fn sync_rate_cards(&self) -> AppResult<i64> {
        info!("Starting rate card synchronization");

        let mut tx = self.pool.begin().await.map_err(|e| {
            error!("Failed to start transaction: {}", e);
            AppError::Transaction(format!("Failed to start transaction: {}", e))
        })?;

        // Step 1: Truncate existing rate cards
        debug!("Truncating rate_cards table");
        sqlx::query("TRUNCATE TABLE rate_cards")
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                error!("Failed to truncate rate_cards: {}", e);
                AppError::Database(format!("Failed to truncate rate_cards: {}", e))
            })?;

        // Step 2: Rebuild rate cards from zones/prefixes/rate_zones
        debug!("Rebuilding rate_cards from zones/prefixes/rate_zones");

        let result = sqlx::query(
            r#"
            INSERT INTO rate_cards (
                rate_name,
                destination_prefix,
                destination_name,
                rate_per_minute,
                billing_increment,
                connection_fee,
                effective_start,
                priority
            )
            SELECT
                COALESCE(rz.rate_name, z.zone_name || ' - ' || p.prefix) as rate_name,
                p.prefix as destination_prefix,
                z.zone_name as destination_name,
                COALESCE(rz.rate_per_minute, 0) as rate_per_minute,
                COALESCE(rz.billing_increment, 6) as billing_increment,
                COALESCE(rz.rate_per_call, 0) as connection_fee,
                COALESCE(rz.effective_from, NOW()) as effective_start,
                COALESCE(rz.priority, 0) as priority
            FROM prefixes p
            INNER JOIN zones z ON p.zone_id = z.id
            INNER JOIN rate_zones rz ON z.id = rz.zone_id
            WHERE rz.enabled = true
                AND z.enabled = true
                AND p.enabled = true
                AND rz.effective_from <= NOW()
            ORDER BY p.prefix
            "#,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("Failed to insert rate cards: {}", e);
            AppError::Database(format!("Failed to insert rate cards: {}", e))
        })?;

        let count = result.rows_affected() as i64;

        // Step 3: Commit transaction
        tx.commit().await.map_err(|e| {
            error!("Failed to commit transaction: {}", e);
            AppError::Transaction(format!("Failed to commit transaction: {}", e))
        })?;

        info!("Rate card synchronization completed: {} cards synced", count);

        Ok(count)
    }

    /// Get synchronization statistics
    ///
    /// Returns counts of zones, prefixes, rate_zones, and rate_cards
    #[instrument(skip(self))]
    pub async fn get_sync_stats(&self) -> AppResult<SyncStats> {
        debug!("Fetching synchronization statistics");

        let zones_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM zones")
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        let prefixes_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM prefixes")
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        let rate_zones_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM rate_zones")
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        let rate_cards_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM rate_cards")
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(SyncStats {
            zones: zones_count.0,
            prefixes: prefixes_count.0,
            rate_zones: rate_zones_count.0,
            rate_cards: rate_cards_count.0,
        })
    }
}

/// Synchronization statistics
#[derive(Debug, Clone)]
pub struct SyncStats {
    pub zones: i64,
    pub prefixes: i64,
    pub rate_zones: i64,
    pub rate_cards: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_stats_creation() {
        let stats = SyncStats {
            zones: 10,
            prefixes: 100,
            rate_zones: 20,
            rate_cards: 100,
        };

        assert_eq!(stats.zones, 10);
        assert_eq!(stats.prefixes, 100);
        assert_eq!(stats.rate_zones, 20);
        assert_eq!(stats.rate_cards, 100);
    }
}
