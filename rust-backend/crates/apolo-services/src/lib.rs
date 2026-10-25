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
// pub mod cdr_generator;
pub mod sip_device;
// pub mod user_service;

// pub use account_service::AccountService;
// pub use authorization::{AuthorizationService, AuthorizationServiceImpl};
// pub use cdr_generator::CdrGenerator;
pub use sip_device::SipDeviceService;
// pub use user_service::UserService;

