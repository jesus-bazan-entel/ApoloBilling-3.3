//! Module catalog and per-user module grants
//!
//! Backs the "which modules can a user see" system: `SystemModule` is the
//! configurable catalog of menu/route modules, and `ModuleGrant` records an
//! explicit grant of one module to one `Usuario` account. See
//! `UserRole::effective_modules` for how these combine into a session's
//! visible module list.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::user::UserRole;

/// A single entry in the module catalog (`system_modules` table)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemModule {
    pub id: i32,
    pub module_key: String,
    pub nombre: String,
    pub seccion: String,
    pub min_role: UserRole,
    pub always_visible: bool,
    pub orden: i32,
    pub activo: bool,
}

/// An explicit module grant to a `Usuario` account (`usuario_modulos` table)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleGrant {
    pub id: i32,
    pub usuario_id: i32,
    pub module_key: String,
    pub otorgado_por: Option<i32>,
    pub otorgado_en: DateTime<Utc>,
}
