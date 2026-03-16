//! ApoloBilling Database Layer
//!
//! This crate provides PostgreSQL database access and repository implementations
//! for the ApoloBilling system. It includes:
//!
//! - Connection pool management with sqlx
//! - Repository implementations for all domain entities
//! - Optimized queries with longest prefix matching for rates
//! - Transaction support for atomic operations

pub mod pool;
pub mod repositories;

pub use pool::create_pool;
pub use repositories::*;

// Re-export commonly used types
pub use apolo_core::{AppError, AppResult};
pub use sqlx::{PgPool, Postgres, Transaction};
