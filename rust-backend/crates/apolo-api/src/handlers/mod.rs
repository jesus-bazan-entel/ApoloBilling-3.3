//! HTTP request handlers

pub mod account;
pub mod active_call;
pub mod audit;
pub mod auth;
pub mod cdr;
pub mod dashboard;
pub mod dialplan;
pub mod management;
pub mod plan;
pub mod rate;
pub mod rate_card;
pub mod reservation;
pub mod stats;
pub mod user;
pub mod ws;

pub use account::configure as configure_accounts;
pub use dialplan::configure as configure_dialplan;
pub use active_call::configure as configure_active_calls;
pub use active_call::create_cdr;
pub use audit::configure as configure_audit;
pub use auth::configure as configure_auth;
pub use cdr::*;
pub use dashboard::configure as configure_dashboard;
pub use management::configure as configure_management;
pub use plan::configure as configure_plans;
pub use rate::configure as configure_rates;
pub use rate_card::configure as configure_rate_cards;
pub use reservation::configure as configure_reservations;
pub use stats::configure as configure_stats;
pub use user::configure as configure_users;
pub use ws::ws_handler;
