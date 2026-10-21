//! Data Transfer Objects (DTOs) for API requests and responses

pub mod account;
pub mod active_call;
pub mod audit;
pub mod auth;
pub mod carriers;
pub mod cdr;
pub mod common;
pub mod endpoint_monitor;
pub mod inbound;
pub mod internal_route;
pub mod management;
pub mod minute_bucket;
pub mod module;
pub mod outbound;
/// PBX groups (same-owner failover sets)
pub mod pbx_group;
pub mod plan;
pub mod rate_card;
pub mod routing;
pub mod sip_endpoint;
pub mod stats;
pub mod user;

pub use account::*;
pub use active_call::*;
pub use audit::*;
pub use auth::*;
pub use carriers::*;
pub use cdr::*;
pub use common::*;
pub use endpoint_monitor::*;
pub use inbound::*;
pub use internal_route::*;
pub use management::*;
pub use minute_bucket::*;
pub use module::*;
pub use outbound::*;
pub use pbx_group::*;
pub use plan::*;
pub use rate_card::*;
pub use routing::*;
pub use sip_endpoint::*;
pub use stats::*;
pub use user::*;
