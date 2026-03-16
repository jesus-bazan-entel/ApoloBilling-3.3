//! Rating service implementation
//!
//! Provides rate lookup and cost calculation with Redis caching for performance.

use apolo_cache::RedisCache;
use apolo_core::{
    models::RateCard,
    traits::{CacheService, RateRepository, RatingService},
    AppError, AppResult,
};
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::sync::Arc;
use tracing::{debug, error, instrument, warn};

use crate::constants::RATE_CACHE_TTL;

/// Redis cache key prefix for rates
const RATE_CACHE_PREFIX: &str = "rate:";

/// Rating service implementation with caching
///
/// Provides high-performance rate lookups using Longest Prefix Match (LPM)
/// with Redis caching to minimize database queries.
pub struct RatingServiceImpl<R: RateRepository> {
    rate_repo: Arc<R>,
    cache: Arc<RedisCache>,
}

impl<R: RateRepository> RatingServiceImpl<R> {
    /// Create a new rating service
    pub fn new(rate_repo: Arc<R>, cache: Arc<RedisCache>) -> Self {
        Self { rate_repo, cache }
    }

    /// Generate cache key for a destination
    fn cache_key(destination: &str) -> String {
        format!("{}{}", RATE_CACHE_PREFIX, destination)
    }

    /// Try to get rate from cache
    async fn get_from_cache(&self, destination: &str) -> AppResult<Option<RateCard>> {
        let key = Self::cache_key(destination);

        match self.cache.get::<RateCard>(&key).await {
            Ok(rate) => {
                if rate.is_some() {
                    debug!("Rate cache HIT for destination: {}", destination);
                }
                Ok(rate)
            }
            Err(e) => {
                warn!("Cache error for destination {}: {}", destination, e);
                // Don't fail on cache errors, just continue without cache
                Ok(None)
            }
        }
    }

    /// Store rate in cache
    async fn store_in_cache(&self, destination: &str, rate: &RateCard) -> AppResult<()> {
        let key = Self::cache_key(destination);

        if let Err(e) = self.cache.set(&key, rate, RATE_CACHE_TTL).await {
            warn!("Failed to cache rate for {}: {}", destination, e);
            // Don't fail on cache errors
        }

        Ok(())
    }
}

#[async_trait]
impl<R: RateRepository> RatingService for RatingServiceImpl<R> {
    #[instrument(skip(self))]
    async fn find_rate(&self, destination: &str) -> AppResult<Option<RateCard>> {
        debug!("Finding rate for destination: {}", destination);

        // Normalize destination
        let normalized = RateCard::normalize_destination(destination);

        if normalized.is_empty() {
            warn!("Empty destination after normalization: {}", destination);
            return Ok(None);
        }

        // Try cache first
        if let Some(rate) = self.get_from_cache(&normalized).await? {
            return Ok(Some(rate));
        }

        // Cache miss - query database
        debug!("Rate cache MISS for destination: {}", normalized);
        let rate = self.rate_repo.find_by_destination(&normalized).await?;

        // Store in cache for future lookups
        if let Some(ref r) = rate {
            self.store_in_cache(&normalized, r).await?;
        }

        Ok(rate)
    }

    #[instrument(skip(self))]
    async fn calculate_cost(&self, destination: &str, duration_seconds: i32) -> AppResult<Decimal> {
        debug!(
            "Calculating cost for destination: {}, duration: {}s",
            destination, duration_seconds
        );

        if duration_seconds <= 0 {
            debug!("Zero duration, returning zero cost");
            return Ok(Decimal::ZERO);
        }

        // Find the rate
        let rate = self
            .find_rate(destination)
            .await?
            .ok_or_else(|| {
                error!("No rate found for destination: {}", destination);
                AppError::RateNotFound(destination.to_string())
            })?;

        // Calculate cost using the rate card
        let cost = rate.calculate_cost(duration_seconds);

        debug!(
            "Calculated cost: {} for {}s at {}/min",
            cost, duration_seconds, rate.rate_per_minute
        );

        Ok(cost)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apolo_core::{models::RateCard, traits::Repository};
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use std::sync::Arc;

    struct MockRateRepository {
        rate: Option<RateCard>,
    }

    #[async_trait]
    impl Repository<RateCard, i32> for MockRateRepository {
        async fn find_by_id(&self, _id: i32) -> AppResult<Option<RateCard>> {
            Ok(self.rate.clone())
        }

        async fn find_all(&self, _limit: i64, _offset: i64) -> AppResult<Vec<RateCard>> {
            Ok(vec![])
        }

        async fn count(&self) -> AppResult<i64> {
            Ok(0)
        }

        async fn create(&self, entity: &RateCard) -> AppResult<RateCard> {
            Ok(entity.clone())
        }

        async fn update(&self, entity: &RateCard) -> AppResult<RateCard> {
            Ok(entity.clone())
        }

        async fn delete(&self, _id: i32) -> AppResult<bool> {
            Ok(true)
        }
    }

    #[async_trait]
    impl RateRepository for MockRateRepository {
        async fn find_by_destination(&self, _destination: &str) -> AppResult<Option<RateCard>> {
            Ok(self.rate.clone())
        }

        async fn search(
            &self,
            _prefix: Option<&str>,
            _name: Option<&str>,
            _limit: i64,
            _offset: i64,
        ) -> AppResult<(Vec<RateCard>, i64)> {
            Ok((vec![], 0))
        }

        async fn bulk_create(&self, _rates: &[RateCard]) -> AppResult<usize> {
            Ok(0)
        }
    }

    #[tokio::test]
    #[ignore] // Requires Redis
    async fn test_calculate_cost_with_rate() {
        let mock_rate = RateCard {
            id: 1,
            rate_name: Some("Test Rate".to_string()),
            destination_prefix: "51".to_string(),
            destination_name: "Peru".to_string(),
            rate_per_minute: dec!(0.10),
            billing_increment: 6,
            connection_fee: dec!(0.00),
            effective_start: Utc::now(),
            effective_end: None,
            priority: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let repo = Arc::new(MockRateRepository {
            rate: Some(mock_rate),
        });
        let cache = Arc::new(RedisCache::new("redis://127.0.0.1:6379").await.unwrap());
        let service = RatingServiceImpl::new(repo, cache);

        let cost = service.calculate_cost("51999888777", 60).await.unwrap();
        assert_eq!(cost, dec!(0.10)); // 1 minute at $0.10/min
    }

    #[test]
    fn test_cache_key() {
        let key = RatingServiceImpl::<MockRateRepository>::cache_key("51999");
        assert_eq!(key, "rate:51999");
    }
}
