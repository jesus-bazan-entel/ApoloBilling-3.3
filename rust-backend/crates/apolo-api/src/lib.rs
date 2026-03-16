//! API layer for ApoloBilling
//!
//! HTTP API handlers for managing billing operations, CDRs, accounts, and more.

#![forbid(unsafe_code)]
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    missing_docs
)]

pub mod dto;
pub mod handlers;

// Re-export DTOs (common types)
pub use dto::{ApiResponse, PaginationParams};

// Re-export handler configuration functions and CDR handlers
pub use handlers::{
    configure_accounts, configure_active_calls, configure_auth, configure_management,
    configure_rate_cards, configure_rates, create_cdr,
    // CDR handlers (configured manually in main.rs)
    cdr,
};
