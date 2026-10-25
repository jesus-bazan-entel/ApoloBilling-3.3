//! Application configuration
//!
//! This module provides centralized configuration management using the `config` crate.
//! Configuration can be loaded from environment variables and config files.

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::env;

/// Main application configuration
#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub auth: AuthConfig,
    pub freeswitch: FreeSwitchConfig,
}

/// HTTP server configuration
#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    /// Server host address
    #[serde(default = "default_host")]
    pub host: String,

    /// Server port
    #[serde(default = "default_port")]
    pub port: u16,

    /// Number of worker threads
    #[serde(default = "default_workers")]
    pub workers: usize,

    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_workers() -> usize {
    num_cpus::get()
}

fn default_timeout() -> u64 {
    30
}

/// Database configuration
#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    /// PostgreSQL connection URL
    pub url: String,

    /// Maximum number of connections in the pool
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Minimum number of connections in the pool
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,

    /// Connection acquire timeout in seconds
    #[serde(default = "default_acquire_timeout")]
    pub acquire_timeout_secs: u64,

    /// Idle connection timeout in seconds
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
}

fn default_max_connections() -> u32 {
    10
}

fn default_min_connections() -> u32 {
    2
}

fn default_acquire_timeout() -> u64 {
    30
}

fn default_idle_timeout() -> u64 {
    600
}

/// Redis configuration
#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    /// Redis connection URL
    pub url: String,

    /// Connection pool size
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,

    /// Default TTL for cached items in seconds
    #[serde(default = "default_cache_ttl")]
    pub default_ttl_secs: u64,
}

fn default_pool_size() -> u32 {
    5
}

fn default_cache_ttl() -> u64 {
    300
}

/// Authentication configuration
#[derive(Debug, Deserialize, Clone)]
pub struct AuthConfig {
    /// JWT signing secret
    pub jwt_secret: String,

    /// JWT token expiration in minutes
    #[serde(default = "default_jwt_expiration")]
    pub jwt_expiration_minutes: i64,

    /// Superadmin password for initial setup
    pub superadmin_password: Option<String>,

    /// Password hashing cost (Argon2 time cost)
    #[serde(default = "default_hash_cost")]
    pub hash_cost: u32,
}

fn default_jwt_expiration() -> i64 {
    1440 // 24 hours
}

fn default_hash_cost() -> u32 {
    3
}

/// FreeSWITCH ESL configuration
#[derive(Debug, Deserialize, Clone)]
pub struct FreeSwitchConfig {
    /// List of FreeSWITCH servers
    #[serde(default)]
    pub servers: Vec<FreeSwitchServer>,

    /// ESL server mode port (when no external servers)
    #[serde(default = "default_esl_port")]
    pub server_port: u16,

    /// Reconnection delay in seconds
    #[serde(default = "default_reconnect_delay")]
    pub reconnect_delay_secs: u64,
}

fn default_esl_port() -> u16 {
    8021
}

fn default_reconnect_delay() -> u64 {
    5
}

/// Individual FreeSWITCH server configuration
#[derive(Debug, Deserialize, Clone)]
pub struct FreeSwitchServer {
    /// Server hostname or IP
    pub host: String,

    /// ESL port
    #[serde(default = "default_fs_port")]
    pub port: u16,

    /// ESL password
    pub password: String,

    /// Server identifier
    pub id: Option<String>,
}

fn default_fs_port() -> u16 {
    8021
}

impl AppConfig {
    /// Load configuration from environment and optional config file
    pub fn load() -> Result<Self, ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".to_string());

        let config = Config::builder()
            // Start with default values
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 8080)?
            .set_default("server.workers", num_cpus::get() as i64)?
            .set_default("server.timeout_secs", 30)?
            .set_default("database.max_connections", 10)?
            .set_default("database.min_connections", 2)?
            .set_default("redis.pool_size", 5)?
            .set_default("redis.default_ttl_secs", 300)?
            .set_default("auth.jwt_expiration_minutes", 1440)?
            .set_default("auth.hash_cost", 3)?
            .set_default("freeswitch.server_port", 8021)?
            .set_default("freeswitch.reconnect_delay_secs", 5)?
            // Load config file if exists
            .add_source(File::with_name("config/default").required(false))
            .add_source(File::with_name(&format!("config/{}", run_mode)).required(false))
            // Load from environment variables with APOLO_ prefix
            .add_source(
                Environment::with_prefix("APOLO")
                    .separator("__")
                    .try_parsing(true),
            )
            // Support legacy environment variables
            .add_source(Environment::default().try_parsing(true))
            .build()?;

        config.try_deserialize()
    }

    /// Load configuration from a specific file
    pub fn from_file(path: &str) -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_source(File::with_name(path))
            .add_source(Environment::with_prefix("APOLO").separator("__"))
            .build()?;

        config.try_deserialize()
    }

    /// Get the server bind address
    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
}
