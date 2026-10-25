//! Repository implementations
//!
//! This module contains concrete implementations of all repository traits
//! defined in apolo-core, using sqlx for PostgreSQL access.

pub mod account_repo;
pub mod audit_repo;
pub mod cdr_repo;
pub mod sip_device_repo;
pub mod user_repo;

pub use account_repo::PgAccountRepository;
pub use audit_repo::PgAuditLogRepository;
pub use cdr_repo::PgCdrRepository;
pub use sip_device_repo::{PgFreeswitchIpRepository, PgSipDeviceRepository};
pub use user_repo::PgUserRepository;
