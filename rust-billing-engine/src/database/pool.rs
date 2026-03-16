// src/database/pool.rs
use deadpool_postgres::{Config, Manager, ManagerConfig, Pool, RecyclingMethod, Runtime};
use tokio_postgres::NoTls;
use tracing::info;

pub type DbPool = Pool;

pub async fn create_pool(database_url: &str) -> Result<Pool, Box<dyn std::error::Error>> {
    // Parse database URL
    let url = database_url
        .replace("postgresql+asyncpg://", "postgresql://")
        .replace("postgres+asyncpg://", "postgresql://");
    
    let mut cfg = Config::new();
    cfg.url = Some(url);
    cfg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });

    let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;
    
    // Test connection
    let client = pool.get().await?;
    let row = client.query_one("SELECT 1 as test", &[]).await?;
    let test: i32 = row.get(0);
    
    if test == 1 {
        info!("Database connection test successful");
    }
    
    Ok(pool)
}
