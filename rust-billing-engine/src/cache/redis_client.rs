// src/cache/redis_client.rs
use redis::{Client, aio::ConnectionManager, AsyncCommands, RedisError};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error};
use crate::error::BillingError;

#[derive(Clone)]
pub struct RedisClient {
    manager: Arc<Mutex<ConnectionManager>>,
}

impl RedisClient {
    pub async fn new(redis_url: &str) -> Result<Self, RedisError> {
        let client = Client::open(redis_url)?;
        let manager = ConnectionManager::new(client).await?;
        
        // Test connection
        let mut conn = manager.clone();
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        
        Ok(Self {
            manager: Arc::new(Mutex::new(manager)),
        })
    }

    // Helper to get a connection from the manager
    async fn get_connection(&self) -> Result<ConnectionManager, BillingError> {
        let manager_guard = self.manager.lock().await;
        Ok(manager_guard.clone())
    }

    pub async fn check_connection(&self) -> Result<(), BillingError> {
        let mut conn = self.get_connection().await?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await
            .map_err(|e| BillingError::Cache(e.to_string()))?;
        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>, RedisError> {
        let mut conn = self.manager.lock().await;
        debug!("Redis GET: {}", key);
        conn.get(key).await
    }

    pub async fn set(&self, key: &str, value: &str, ttl: usize) -> Result<(), BillingError> {
        let mut conn = self.get_connection().await?;
        conn.set_ex(key, value, ttl as u64).await
            .map_err(|e| BillingError::Cache(e.to_string()))
    }

    pub async fn delete(&self, key: &str) -> Result<(), RedisError> {
        let mut conn = self.manager.lock().await;
        debug!("Redis DEL: {}", key);
        conn.del(key).await
    }

    pub async fn sadd(&self, key: &str, member: &str) -> Result<(), RedisError> {
        let mut conn = self.manager.lock().await;
        debug!("Redis SADD: {} -> {}", key, member);
        conn.sadd(key, member).await
    }

    pub async fn srem(&self, key: &str, member: &str) -> Result<(), RedisError> {
        let mut conn = self.manager.lock().await;
        debug!("Redis SREM: {} -> {}", key, member);
        conn.srem(key, member).await
    }

    pub async fn scard(&self, key: &str) -> Result<i32, RedisError> {
        let mut conn = self.manager.lock().await;
        debug!("Redis SCARD: {}", key);
        conn.scard(key).await
    }

    pub async fn exists(&self, key: &str) -> Result<bool, RedisError> {
        let mut conn = self.manager.lock().await;
        conn.exists(key).await
    }

    pub async fn expire(&self, key: &str, seconds: usize) -> Result<(), BillingError> {
        let mut conn = self.get_connection().await?;
        conn.expire(key, seconds as i64).await
            .map_err(|e| BillingError::Cache(e.to_string()))
    }

    /// SET if Not eXists with TTL - returns true if key was set, false if it already existed
    pub async fn setnx_ex(&self, key: &str, value: &str, ttl: usize) -> Result<bool, BillingError> {
        let mut conn = self.get_connection().await?;
        debug!("Redis SETNX: {} (TTL: {}s)", key, ttl);
        // SET key value EX ttl NX - returns OK if set, nil if key exists
        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg(value)
            .arg("EX")
            .arg(ttl)
            .arg("NX")
            .query_async(&mut conn)
            .await
            .map_err(|e| BillingError::Cache(e.to_string()))?;

        Ok(result.is_some())
    }
}
