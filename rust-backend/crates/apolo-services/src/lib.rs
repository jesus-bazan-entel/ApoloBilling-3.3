//! Business logic services for ApoloBilling
//!
//! This crate contains all the business logic services that orchestrate
//! the billing operations, including rating, authorization, reservation
//! management, and CDR generation.
//!
//! # Architecture
//!
//! Services are designed to be composable and testable:
//! - Each service owns its dependencies (repositories, cache, etc.)
//! - Services are wrapped in Arc for safe sharing across async tasks
//! - All operations are instrumented with tracing
//! - Comprehensive error handling with AppError
//!
//! # Services
//!
//! - `RatingServiceImpl` - Rate lookup and cost calculation with caching
//! - `BillingSyncService` - Rate card synchronization from zones/prefixes
//! - `AuthorizationService` - Call authorization with balance reservation
//! - `ReservationManager` - Balance reservation lifecycle management
//! - `AccountService` - Account CRUD and balance operations
//! - `UserService` - User management and authentication
//! - `CdrGenerator` - CDR generation from call events

// NOTE: Some modules are commented out as their files don't exist yet
// pub mod account_service;
// pub mod authorization;
pub mod billing_sync;
// pub mod cdr_generator;
pub mod rating;
pub mod reservation_manager;
// pub mod user_service;

// pub use account_service::AccountService;
// pub use authorization::{AuthorizationService, AuthorizationServiceImpl};
pub use billing_sync::BillingSyncService;
// pub use cdr_generator::CdrGenerator;
pub use rating::RatingServiceImpl;
pub use reservation_manager::ReservationManager;
// pub use user_service::UserService;

/// Business logic constants
pub mod constants {
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    /// Initial reservation duration in minutes
    pub const INITIAL_RESERVATION_MINUTES: i32 = 5;

    /// Reservation buffer percentage (to handle rate variations)
    pub const RESERVATION_BUFFER_PERCENT: i32 = 8;

    /// Minimum reservation amount in USD
    pub const MIN_RESERVATION: Decimal = dec!(0.30);

    /// Maximum reservation amount in USD
    pub const MAX_RESERVATION: Decimal = dec!(30.00);

    /// Reservation TTL in seconds (45 minutes)
    pub const RESERVATION_TTL: i64 = 2700;

    /// Maximum concurrent calls per account
    pub const MAX_CONCURRENT_CALLS: i32 = 5;

    /// Maximum deficit amount allowed (negative balance)
    pub const MAX_DEFICIT: Decimal = dec!(10.00);

    /// Rate cache TTL in seconds (5 minutes)
    pub const RATE_CACHE_TTL: u64 = 300;
}
