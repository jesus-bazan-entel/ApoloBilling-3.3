// src/config.rs
use serde::Deserialize;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub environment: String,
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub redis_url: String,
    pub freeswitch_servers: Vec<FreeSwitchServer>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FreeSwitchServer {
    pub host: String,
    pub port: u16,
    pub password: String,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        dotenv::dotenv().ok();

        let freeswitch_servers = Self::parse_freeswitch_servers(
            &env::var("FREESWITCH_SERVERS").unwrap_or_default()
        )?;

        Ok(Config {
            environment: env::var("ENVIRONMENT")
                .unwrap_or_else(|_| "production".to_string()),
            host: env::var("HOST")
                .unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "9000".to_string())
                .parse()?,
            database_url: env::var("DATABASE_URL")?,
            redis_url: env::var("REDIS_URL")?,
            freeswitch_servers,
        })
    }

    fn parse_freeswitch_servers(
        servers_str: &str
    ) -> Result<Vec<FreeSwitchServer>, Box<dyn std::error::Error>> {
        if servers_str.is_empty() {
            return Ok(Vec::new());
        }

        let mut servers = Vec::new();
        
        for server_config in servers_str.split(',') {
            let parts: Vec<&str> = server_config.trim().split(':').collect();
            if parts.len() == 3 {
                servers.push(FreeSwitchServer {
                    host: parts[0].to_string(),
                    port: parts[1].parse()?,
                    password: parts[2].to_string(),
                });
            }
        }

        Ok(servers)
    }
}
