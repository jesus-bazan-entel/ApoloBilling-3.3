//! Redis caching layer for ApoloBilling
//!
//! Provides a high-performance caching implementation using Redis with connection pooling.
//! Implements the `CacheService` trait from apolo-core for rate limiting, session management,
//! and balance reservation tracking.
//!
//! # Features
//!
//! - Connection pooling via Redis ConnectionManager
//! - Automatic serialization/deserialization using serde_json
//! - TTL support for cache entries
//! - Set operations for tracking active sessions and reservations
//! - Comprehensive error handling with conversion to AppError
//!
//! # Example
//!
//! ```no_run
//! use apolo_cache::RedisCache;
//! use apolo_core::traits::CacheService;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let cache = RedisCache::new("redis://127.0.0.1:6379").await?;
//!
//!     // Set a value with 60 second TTL
//!     cache.set("my_key", &"my_value", 60).await?;
//!
//!     // Get the value back
//!     let value: Option<String> = cache.get("my_key").await?;
//!     assert_eq!(value, Some("my_value".to_string()));
//!
//!     Ok(())
//! }
//! ```

pub mod keys;

use apolo_core::error::AppError;
use apolo_core::traits::CacheService;
use async_trait::async_trait;
use redis::{aio::ConnectionManager, AsyncCommands, Client, RedisError};
use serde::{de::DeserializeOwned, Serialize};
use tracing::{debug, error, warn};

/// Redis cache implementation with connection pooling
///
/// Wraps a Redis ConnectionManager to provide efficient, multiplexed access
/// to Redis. All operations are async and return Results with AppError.
#[derive(Clone)]
pub struct RedisCache {
    manager: ConnectionManager,
}

impl RedisCache {
    /// Create a new Redis cache instance
    ///
    /// # Arguments
    ///
    /// * `url` - Redis connection URL (e.g., "redis://127.0.0.1:6379")
    ///
    /// # Errors
    ///
    /// Returns `AppError::CacheConnection` if the connection fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use apolo_cache::RedisCache;
    /// # async fn example() -> Result<(), apolo_core::error::AppError> {
    /// let cache = RedisCache::new("redis://localhost:6379").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(url: &str) -> Result<Self, AppError> {
        debug!("Connecting to Redis at {}", url);

        let client = Client::open(url).map_err(|e| {
            error!("Failed to create Redis client: {}", e);
            AppError::CacheConnection(format!("Invalid Redis URL: {}", e))
        })?;

        let manager = ConnectionManager::new(client).await.map_err(|e| {
            error!("Failed to establish Redis connection: {}", e);
            AppError::CacheConnection(format!("Connection failed: {}", e))
        })?;

        debug!("Redis connection established successfully");
        Ok(Self { manager })
    }

    /// Ping the Redis server to check connectivity
    ///
    /// # Errors
    ///
    /// Returns `AppError::Cache` if the ping fails
    pub async fn ping(&self) -> Result<(), AppError> {
        let mut conn = self.manager.clone();
        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                error!("Redis ping failed: {}", e);
                AppError::Cache(format!("Ping failed: {}", e))
            })?;
        Ok(())
    }

    /// Flush all keys from the current database
    ///
    /// # Warning
    ///
    /// This operation is destructive and will delete all cached data.
    /// Use only in testing or development environments.
    ///
    /// # Errors
    ///
    /// Returns `AppError::Cache` if the flush operation fails
    #[cfg(test)]
    pub async fn flush_db(&self) -> Result<(), AppError> {
        let mut conn = self.manager.clone();
        let _: () = redis::cmd("FLUSHDB")
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                error!("Failed to flush database: {}", e);
                AppError::Cache(format!("Flush failed: {}", e))
            })?;
        Ok(())
    }

    /// Convert RedisError to AppError
    fn map_redis_error(err: RedisError) -> AppError {
        match err.kind() {
            redis::ErrorKind::IoError => {
                error!("Redis I/O error: {}", err);
                AppError::CacheConnection(format!("I/O error: {}", err))
            }
            redis::ErrorKind::TypeError => {
                warn!("Redis type error: {}", err);
                AppError::Cache(format!("Type mismatch: {}", err))
            }
            _ => {
                error!("Redis error: {}", err);
                AppError::Cache(err.to_string())
            }
        }
    }
}

#[async_trait]
impl CacheService for RedisCache {
    /// Get a value from cache and deserialize it
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type to deserialize to, must implement `DeserializeOwned`
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to retrieve
    ///
    /// # Returns
    ///
    /// - `Ok(Some(T))` if the key exists and deserialization succeeds
    /// - `Ok(None)` if the key doesn't exist
    /// - `Err(AppError)` if Redis or deserialization fails
    async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, AppError> {
        debug!("GET {}", key);
        let mut conn = self.manager.clone();

        let result: Option<String> = conn.get(key).await.map_err(Self::map_redis_error)?;

        match result {
            Some(json) => {
                let value = serde_json::from_str::<T>(&json).map_err(|e| {
                    error!("Failed to deserialize value for key {}: {}", key, e);
                    AppError::Serialization(format!("Deserialization failed: {}", e))
                })?;
                debug!("Cache HIT: {}", key);
                Ok(Some(value))
            }
            None => {
                debug!("Cache MISS: {}", key);
                Ok(None)
            }
        }
    }

    /// Set a value in cache with TTL
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type to serialize, must implement `Serialize + Send + Sync`
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to set
    /// * `value` - The value to cache
    /// * `ttl_secs` - Time-to-live in seconds
    ///
    /// # Errors
    ///
    /// Returns `AppError` if serialization or Redis operation fails
    async fn set<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
        ttl_secs: u64,
    ) -> Result<(), AppError> {
        debug!("SET {} (TTL: {}s)", key, ttl_secs);
        let mut conn = self.manager.clone();

        let json = serde_json::to_string(value).map_err(|e| {
            error!("Failed to serialize value for key {}: {}", key, e);
            AppError::Serialization(format!("Serialization failed: {}", e))
        })?;

        let _: () = conn
            .set_ex(key, json, ttl_secs)
            .await
            .map_err(Self::map_redis_error)?;

        Ok(())
    }

    /// Delete a key from cache
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to delete
    ///
    /// # Returns
    ///
    /// `Ok(true)` if the key was deleted, `Ok(false)` if it didn't exist
    async fn delete(&self, key: &str) -> Result<bool, AppError> {
        debug!("DEL {}", key);
        let mut conn = self.manager.clone();

        let deleted: i32 = conn.del(key).await.map_err(Self::map_redis_error)?;

        Ok(deleted > 0)
    }

    /// Check if a key exists in cache
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to check
    ///
    /// # Returns
    ///
    /// `Ok(true)` if the key exists, `Ok(false)` otherwise
    async fn exists(&self, key: &str) -> Result<bool, AppError> {
        debug!("EXISTS {}", key);
        let mut conn = self.manager.clone();

        let exists: bool = conn.exists(key).await.map_err(Self::map_redis_error)?;

        Ok(exists)
    }

    /// Add a member to a set
    ///
    /// # Arguments
    ///
    /// * `key` - The set key
    /// * `member` - The member to add
    ///
    /// # Returns
    ///
    /// `Ok(true)` if the member was added, `Ok(false)` if it already existed
    async fn sadd(&self, key: &str, member: &str) -> Result<bool, AppError> {
        debug!("SADD {} {}", key, member);
        let mut conn = self.manager.clone();

        let added: i32 = conn
            .sadd(key, member)
            .await
            .map_err(Self::map_redis_error)?;

        Ok(added > 0)
    }

    /// Remove a member from a set
    ///
    /// # Arguments
    ///
    /// * `key` - The set key
    /// * `member` - The member to remove
    ///
    /// # Returns
    ///
    /// `Ok(true)` if the member was removed, `Ok(false)` if it didn't exist
    async fn srem(&self, key: &str, member: &str) -> Result<bool, AppError> {
        debug!("SREM {} {}", key, member);
        let mut conn = self.manager.clone();

        let removed: i32 = conn
            .srem(key, member)
            .await
            .map_err(Self::map_redis_error)?;

        Ok(removed > 0)
    }

    /// Count the number of members in a set
    ///
    /// # Arguments
    ///
    /// * `key` - The set key
    ///
    /// # Returns
    ///
    /// The number of members in the set
    async fn scard(&self, key: &str) -> Result<i64, AppError> {
        debug!("SCARD {}", key);
        let mut conn = self.manager.clone();

        let count: i64 = conn.scard(key).await.map_err(Self::map_redis_error)?;

        Ok(count)
    }

    /// Set expiration time on a key
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key
    /// * `ttl_secs` - Time-to-live in seconds
    ///
    /// # Returns
    ///
    /// `Ok(true)` if the timeout was set, `Ok(false)` if the key doesn't exist
    async fn expire(&self, key: &str, ttl_secs: u64) -> Result<bool, AppError> {
        debug!("EXPIRE {} {}", key, ttl_secs);
        let mut conn = self.manager.clone();

        let result: bool = conn
            .expire(key, ttl_secs as i64)
            .await
            .map_err(Self::map_redis_error)?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestData {
        id: i32,
        name: String,
    }

    async fn setup_cache() -> RedisCache {
        let cache = RedisCache::new("redis://127.0.0.1:6379")
            .await
            .expect("Failed to connect to Redis");
        cache.flush_db().await.expect("Failed to flush DB");
        cache
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_ping() {
        let cache = setup_cache().await;
        assert!(cache.ping().await.is_ok());
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_set_and_get() {
        let cache = setup_cache().await;

        let data = TestData {
            id: 1,
            name: "Test".to_string(),
        };

        // Set value
        cache.set("test_key", &data, 60).await.unwrap();

        // Get value back
        let result: Option<TestData> = cache.get("test_key").await.unwrap();
        assert_eq!(result, Some(data));
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_get_nonexistent() {
        let cache = setup_cache().await;

        let result: Option<TestData> = cache.get("nonexistent").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_delete() {
        let cache = setup_cache().await;

        let data = TestData {
            id: 1,
            name: "Test".to_string(),
        };

        // Set and verify
        cache.set("test_key", &data, 60).await.unwrap();
        assert!(cache.exists("test_key").await.unwrap());

        // Delete and verify
        let deleted = cache.delete("test_key").await.unwrap();
        assert!(deleted);
        assert!(!cache.exists("test_key").await.unwrap());

        // Delete nonexistent
        let deleted = cache.delete("test_key").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_set_operations() {
        let cache = setup_cache().await;

        // Add members
        let added = cache.sadd("test_set", "member1").await.unwrap();
        assert!(added);

        let added = cache.sadd("test_set", "member2").await.unwrap();
        assert!(added);

        // Adding same member again should return false
        let added = cache.sadd("test_set", "member1").await.unwrap();
        assert!(!added);

        // Count members
        let count = cache.scard("test_set").await.unwrap();
        assert_eq!(count, 2);

        // Remove member
        let removed = cache.srem("test_set", "member1").await.unwrap();
        assert!(removed);

        let count = cache.scard("test_set").await.unwrap();
        assert_eq!(count, 1);

        // Remove nonexistent member
        let removed = cache.srem("test_set", "member3").await.unwrap();
        assert!(!removed);
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_expire() {
        let cache = setup_cache().await;

        let data = TestData {
            id: 1,
            name: "Test".to_string(),
        };

        // Set without expiration
        cache.set("test_key", &data, 3600).await.unwrap();

        // Set expiration to 1 second
        let result = cache.expire("test_key", 1).await.unwrap();
        assert!(result);

        // Verify key exists
        assert!(cache.exists("test_key").await.unwrap());

        // Wait for expiration
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Verify key is gone
        assert!(!cache.exists("test_key").await.unwrap());
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_ttl_on_set() {
        let cache = setup_cache().await;

        let data = TestData {
            id: 1,
            name: "Test".to_string(),
        };

        // Set with 1 second TTL
        cache.set("test_key", &data, 1).await.unwrap();

        // Verify key exists
        assert!(cache.exists("test_key").await.unwrap());

        // Wait for expiration
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Verify key is gone
        let result: Option<TestData> = cache.get("test_key").await.unwrap();
        assert_eq!(result, None);
    }
}
