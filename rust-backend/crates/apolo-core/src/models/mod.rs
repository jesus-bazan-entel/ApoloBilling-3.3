//! Domain models for ApoloBilling
//!
//! This module contains all the core domain models used throughout the application.

pub mod account;
pub mod audit;
pub mod cdr;
pub mod inbound;
pub mod internal_route;
pub mod minute_bucket;
pub mod module;
pub mod plan;
pub mod rate;
pub mod sip_endpoint;
pub mod user;
pub mod zone;

pub use account::{Account, AccountStatus};
pub use audit::{AuditLog, AuditLogBuilder, AuditLogData};
pub use inbound::{DestinationType, InboundRoute, InboundRouteWithGateway};
// Deprecated type aliases for backward compatibility
pub use cdr::{ActiveCall, Cdr};
pub use inbound::{Carrier, CarrierWithGateway, DidOverride, DidOverrideWithCarrier};
pub use internal_route::{InternalRoute, InternalRouteWithEndpoints, SystemEndpoint};
pub use minute_bucket::{
    AccountBucketConfig, BucketConfig, BucketType, MinuteBucketAddition, MinuteBucketAlert,
    MinuteBucketCycle, MinuteBucketReportRow, PlanBucketConfig, ProrationMode,
};
pub use module::{ModuleGrant, SystemModule};
pub use plan::Plan;
pub use rate::RateCard;
pub use sip_endpoint::{AuthMode, EndpointType, FreeswitchAllowedIp, SipEndpoint};
pub use user::{User, UserInfo, UserRole};
pub use zone::{NetworkType, Prefix, RateZone, Zone, ZoneType};
