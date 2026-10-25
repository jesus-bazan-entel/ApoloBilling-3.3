// src/error.rs
use thiserror::Error;
use actix_web::{http::StatusCode, ResponseError, HttpResponse};
use serde_json::json;

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Database error: {0}")]
    Database(#[from] tokio_postgres::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl ResponseError for EngineError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(json!({
            "error": self.error_code(),
            "message": self.to_string(),
        }))
    }

    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

impl EngineError {
    fn error_code(&self) -> &str {
        match self {
            EngineError::Database(_) => "database_error",
            EngineError::Internal(_) => "internal_error",
        }
    }
}
