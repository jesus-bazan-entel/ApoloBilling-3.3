//! ApoloBilling Core Library
//!
//! This crate provides the foundational types, traits, and error handling
//! for the ApoloBilling system. It includes:
//!
//! - Domain models (Account, RateCard, CDR, etc.)
//! - Common traits for repositories and services
//! - Unified error handling with HTTP response mapping
//! - Application configuration

pub mod config;
pub mod error;
pub mod models;
pub mod traits;

pub use config::AppConfig;
pub use error::AppError;

/// Result type alias using AppError
pub type AppResult<T> = Result<T, AppError>;
