//! API layer for Apolo SBC
//!
//! HTTP API handlers for CDRs, customers, SIP devices, routing and monitoring.

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
    configure_accounts, configure_active_calls, configure_auth, create_cdr,
    // CDR handlers (configured manually in main.rs)
    cdr,
};
