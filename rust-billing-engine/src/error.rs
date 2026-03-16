// src/error.rs
use thiserror::Error;
use actix_web::{http::StatusCode, ResponseError, HttpResponse};
use serde_json::json;

#[derive(Error, Debug)]
pub enum BillingError {
    #[error("Database error: {0}")]
    Database(#[from] tokio_postgres::Error),
    
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Cache error: {0}")]
    Cache(String),
    
    #[error("Account not found")]
    AccountNotFound,
    
    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance {
        required: String,
        available: String,
    },
    
    #[error("Rate not found for destination: {0}")]
    RateNotFound(String),
    
    #[error("Reservation failed: {0}")]
    ReservationFailed(String),
    
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    
    #[error("Concurrent limit exceeded")]
    ConcurrentLimitExceeded,
    
    #[error("Internal error: {0}")]
    Internal(String),
}

impl ResponseError for BillingError {
    fn error_response(&self) -> HttpResponse {
        let status_code = self.status_code();
        
        HttpResponse::build(status_code).json(json!({
            "error": self.error_code(),
            "message": self.to_string(),
        }))
    }

    fn status_code(&self) -> StatusCode {
        match self {
            BillingError::AccountNotFound => StatusCode::NOT_FOUND,
            BillingError::InsufficientBalance { .. } => StatusCode::FORBIDDEN,
            BillingError::RateNotFound(_) => StatusCode::NOT_FOUND,
            BillingError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            BillingError::ConcurrentLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl BillingError {
    fn error_code(&self) -> &str {
        match self {
            BillingError::Database(_) => "database_error",
            BillingError::Redis(_) => "cache_error",
            BillingError::Cache(_) => "cache_error",
            BillingError::AccountNotFound => "account_not_found",
            BillingError::InsufficientBalance { .. } => "insufficient_balance",
            BillingError::RateNotFound(_) => "rate_not_found",
            BillingError::ReservationFailed(_) => "reservation_failed",
            BillingError::InvalidRequest(_) => "invalid_request",
            BillingError::ConcurrentLimitExceeded => "concurrent_limit_exceeded",
            BillingError::Internal(_) => "internal_error",
        }
    }
}
