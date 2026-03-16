//! PostgreSQL connection pool management
//!
//! Provides utilities for creating and managing database connection pools.

use apolo_core::{AppError, AppResult};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::time::Duration;
use tracing::{info, warn};

/// Default maximum number of connections in the pool
const DEFAULT_MAX_CONNECTIONS: u32 = 20;

/// Default connection timeout in seconds
const DEFAULT_CONNECT_TIMEOUT: u64 = 30;

/// Default idle timeout in seconds
const DEFAULT_IDLE_TIMEOUT: u64 = 600;

/// Create a PostgreSQL connection pool
///
/// # Arguments
///
/// * `database_url` - PostgreSQL connection URL (e.g., "postgresql://user:pass@localhost/db")
/// * `max_connections` - Maximum number of connections in the pool (None = default)
///
/// # Returns
///
/// A configured `PgPool` ready for use
///
/// # Example
///
/// ```no_run
/// use apolo_db::create_pool;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let pool = create_pool("postgresql://localhost/apolo_billing", None).await?;
///     Ok(())
/// }
/// ```
pub async fn create_pool(database_url: &str, max_connections: Option<u32>) -> AppResult<PgPool> {
    info!("Creating database connection pool");

    let max_conns = max_connections.unwrap_or(DEFAULT_MAX_CONNECTIONS);

    let pool = PgPoolOptions::new()
        .max_connections(max_conns)
        .acquire_timeout(Duration::from_secs(DEFAULT_CONNECT_TIMEOUT))
        .idle_timeout(Some(Duration::from_secs(DEFAULT_IDLE_TIMEOUT)))
        .test_before_acquire(true)
        .connect(database_url)
        .await
        .map_err(|e| {
            warn!("Failed to create database pool: {}", e);
            AppError::Pool(format!("Failed to connect to database: {}", e))
        })?;

    info!(
        "Database pool created successfully with {} max connections",
        max_conns
    );

    // Test the connection
    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .map_err(|e| AppError::Database(format!("Database health check failed: {}", e)))?;

    info!("Database connection verified");

    Ok(pool)
}

/// Create a connection pool from options
///
/// Provides more fine-grained control over connection parameters.
pub async fn create_pool_with_options(
    options: PgConnectOptions,
    max_connections: u32,
) -> AppResult<PgPool> {
    info!("Creating database connection pool with custom options");

    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(DEFAULT_CONNECT_TIMEOUT))
        .idle_timeout(Some(Duration::from_secs(DEFAULT_IDLE_TIMEOUT)))
        .test_before_acquire(true)
        .connect_with(options)
        .await
        .map_err(|e| {
            warn!("Failed to create database pool: {}", e);
            AppError::Pool(format!("Failed to connect to database: {}", e))
        })?;

    info!("Database pool created successfully");

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_create_pool() {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/apolo_billing".to_string());

        let result = create_pool(&database_url, Some(5)).await;
        assert!(result.is_ok());
    }
}
