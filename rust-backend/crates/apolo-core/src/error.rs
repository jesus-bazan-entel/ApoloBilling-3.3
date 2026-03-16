//! Unified error handling for ApoloBilling
//!
//! This module provides a comprehensive error type that covers all possible
//! failure scenarios in the application, with automatic HTTP response mapping.

use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde_json::json;
use std::fmt;
use thiserror::Error;

/// Main application error type
///
/// All errors in the application should be converted to this type.
/// It implements `ResponseError` for automatic HTTP response generation.
#[derive(Error, Debug)]
pub enum AppError {
    // ==================== Database Errors ====================
    #[error("Database error: {0}")]
    Database(String),

    #[error("Database pool error: {0}")]
    Pool(String),

    #[error("Transaction failed: {0}")]
    Transaction(String),

    // ==================== Cache Errors ====================
    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Cache connection failed: {0}")]
    CacheConnection(String),

    // ==================== Authentication Errors ====================
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Token expired")]
    TokenExpired,

    #[error("Invalid token: {0}")]
    InvalidToken(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: insufficient permissions")]
    Forbidden,

    #[error("Password hashing failed: {0}")]
    PasswordHash(String),

    // ==================== Business Logic Errors ====================
    #[error("Account not found: {0}")]
    AccountNotFound(String),

    #[error("Account suspended: {0}")]
    AccountSuspended(String),

    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: String, available: String },

    #[error("Rate not found for destination: {0}")]
    RateNotFound(String),

    #[error("Reservation not found: {0}")]
    ReservationNotFound(String),

    #[error("Reservation failed: {0}")]
    ReservationFailed(String),

    #[error("Reservation expired: {0}")]
    ReservationExpired(String),

    #[error("Concurrent call limit exceeded: max {max} calls allowed")]
    ConcurrentLimitExceeded { max: i32 },

    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Zone not found: {0}")]
    ZoneNotFound(String),

    #[error("Prefix not found: {0}")]
    PrefixNotFound(String),

    // ==================== Validation Errors ====================
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    // ==================== Resource Errors ====================
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    // ==================== Internal Errors ====================
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    // ==================== External Service Errors ====================
    #[error("ESL connection error: {0}")]
    EslConnection(String),

    #[error("ESL command failed: {0}")]
    EslCommand(String),
}

impl AppError {
    /// Returns the HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            // 400 Bad Request
            AppError::Validation(_) | AppError::InvalidInput(_) | AppError::MissingField(_) => {
                StatusCode::BAD_REQUEST
            }

            // 401 Unauthorized
            AppError::InvalidCredentials | AppError::InvalidToken(_) | AppError::TokenExpired => {
                StatusCode::UNAUTHORIZED
            }

            // 402 Payment Required
            AppError::InsufficientBalance { .. } => StatusCode::PAYMENT_REQUIRED,

            // 403 Forbidden
            AppError::Forbidden | AppError::Unauthorized(_) | AppError::AccountSuspended(_) => {
                StatusCode::FORBIDDEN
            }

            // 404 Not Found
            AppError::AccountNotFound(_)
            | AppError::RateNotFound(_)
            | AppError::ReservationNotFound(_)
            | AppError::UserNotFound(_)
            | AppError::ZoneNotFound(_)
            | AppError::PrefixNotFound(_)
            | AppError::NotFound(_) => StatusCode::NOT_FOUND,

            // 409 Conflict
            AppError::Conflict(_) | AppError::AlreadyExists(_) => StatusCode::CONFLICT,

            // 429 Too Many Requests
            AppError::ConcurrentLimitExceeded { .. } => StatusCode::TOO_MANY_REQUESTS,

            // 500 Internal Server Error
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Returns the error code for API responses
    pub fn error_code(&self) -> &'static str {
        match self {
            AppError::Database(_) => "database_error",
            AppError::Pool(_) => "pool_error",
            AppError::Transaction(_) => "transaction_error",
            AppError::Cache(_) => "cache_error",
            AppError::CacheConnection(_) => "cache_connection_error",
            AppError::InvalidCredentials => "invalid_credentials",
            AppError::TokenExpired => "token_expired",
            AppError::InvalidToken(_) => "invalid_token",
            AppError::Unauthorized(_) => "unauthorized",
            AppError::Forbidden => "forbidden",
            AppError::PasswordHash(_) => "password_error",
            AppError::AccountNotFound(_) => "account_not_found",
            AppError::AccountSuspended(_) => "account_suspended",
            AppError::InsufficientBalance { .. } => "insufficient_balance",
            AppError::RateNotFound(_) => "rate_not_found",
            AppError::ReservationNotFound(_) => "reservation_not_found",
            AppError::ReservationFailed(_) => "reservation_failed",
            AppError::ReservationExpired(_) => "reservation_expired",
            AppError::ConcurrentLimitExceeded { .. } => "concurrent_limit_exceeded",
            AppError::UserNotFound(_) => "user_not_found",
            AppError::ZoneNotFound(_) => "zone_not_found",
            AppError::PrefixNotFound(_) => "prefix_not_found",
            AppError::Validation(_) => "validation_error",
            AppError::InvalidInput(_) => "invalid_input",
            AppError::MissingField(_) => "missing_field",
            AppError::NotFound(_) => "not_found",
            AppError::Conflict(_) => "conflict",
            AppError::AlreadyExists(_) => "already_exists",
            AppError::Internal(_) => "internal_error",
            AppError::Config(_) => "config_error",
            AppError::Serialization(_) => "serialization_error",
            AppError::EslConnection(_) => "esl_connection_error",
            AppError::EslCommand(_) => "esl_command_error",
        }
    }
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        AppError::status_code(self)
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();
        let body = json!({
            "error": self.error_code(),
            "message": self.to_string(),
            "status": status.as_u16(),
        });

        HttpResponse::build(status).json(body)
    }
}

// ==================== From implementations ====================

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Serialization(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<config::ConfigError> for AppError {
    fn from(err: config::ConfigError) -> Self {
        AppError::Config(err.to_string())
    }
}

impl From<validator::ValidationErrors> for AppError {
    fn from(err: validator::ValidationErrors) -> Self {
        AppError::Validation(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_status_codes() {
        assert_eq!(
            AppError::InvalidCredentials.status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AppError::AccountNotFound("123".to_string()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            AppError::InsufficientBalance {
                required: "10.00".to_string(),
                available: "5.00".to_string()
            }
            .status_code(),
            StatusCode::PAYMENT_REQUIRED
        );
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(
            AppError::InvalidCredentials.error_code(),
            "invalid_credentials"
        );
        assert_eq!(
            AppError::ConcurrentLimitExceeded { max: 5 }.error_code(),
            "concurrent_limit_exceeded"
        );
    }
}
