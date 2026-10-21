//! Repository implementations
//!
//! This module contains concrete implementations of all repository traits
//! defined in apolo-core, using sqlx for PostgreSQL access.

pub mod account_repo;
pub mod audit_repo;
pub mod cdr_repo;
pub mod internal_route_repo;
pub mod minute_bucket_repo;
pub mod module_repo;
pub mod plan_repo;
pub mod rate_repo;
pub mod sip_endpoint_repo;
pub mod user_repo;

pub use account_repo::PgAccountRepository;
pub use audit_repo::PgAuditLogRepository;
pub use cdr_repo::PgCdrRepository;
pub use internal_route_repo::{InternalRouteRepository, SystemEndpointRepository};
pub use minute_bucket_repo::PgMinuteBucketRepository;
pub use module_repo::PgModuleRepository;
pub use plan_repo::PgPlanRepository;
pub use rate_repo::PgRateRepository;
pub use sip_endpoint_repo::{PgFreeswitchIpRepository, PgSipEndpointRepository};
pub use user_repo::PgUserRepository;
