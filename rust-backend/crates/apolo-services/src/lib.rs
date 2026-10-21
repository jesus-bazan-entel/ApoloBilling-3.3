//! Business logic services for ApoloBilling
//!
//! This crate contains business logic services that don't fit cleanly as
//! HTTP handlers or repositories.
//!
//! # Services
//!
//! - `SipEndpointService` - SIP endpoint credential encryption
//! - `minute_bucket_alerts` - 80%/100% bucket-consumption email dispatcher

pub mod minute_bucket_alerts;
pub mod sip_endpoint;

pub use sip_endpoint::SipEndpointService;
