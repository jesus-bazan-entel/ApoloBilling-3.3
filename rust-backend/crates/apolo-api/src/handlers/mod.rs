//! HTTP request handlers

pub mod account;
pub mod active_call;
pub mod audit;
pub mod auth;
pub mod cdr;
pub mod dialplan;
pub mod freeswitch_directory;
pub mod internal_routes;
pub mod kamailio_dialplan;
pub mod kamailio_endpoints;
pub mod provisioning;
pub mod settings;
pub mod sip_device;
pub mod stats;
pub mod unified_routing;
pub mod user;
pub mod ws;

pub use account::configure as configure_accounts;
pub use active_call::configure as configure_active_calls;
pub use active_call::create_cdr;
pub use audit::configure as configure_audit;
pub use auth::configure as configure_auth;
pub use cdr::*;
pub use dialplan::configure as configure_dialplan;
pub use freeswitch_directory::configure as configure_freeswitch_directory;
pub use internal_routes::configure_routes as configure_internal_routes;
pub use kamailio_dialplan::configure as configure_kamailio_dialplan;
pub use kamailio_endpoints::configure as configure_kamailio_endpoints;
pub use provisioning::configure as configure_provisioning;
pub use settings::configure as configure_settings;
pub use sip_device::configure as configure_sip_devices;
pub use stats::configure as configure_stats;
pub use unified_routing::configure as configure_unified_routing;
pub use user::configure as configure_users;
pub use ws::ws_handler;
