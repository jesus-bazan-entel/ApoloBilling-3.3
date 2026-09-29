//! Module catalog and per-user module grant DTOs

use apolo_core::models::SystemModule;
use serde::{Deserialize, Serialize};

/// A single catalog entry, as returned by `GET /modules`
#[derive(Debug, Clone, Serialize)]
pub struct ModuleResponse {
    pub module_key: String,
    pub nombre: String,
    pub seccion: String,
    pub min_role: String,
    pub always_visible: bool,
    pub orden: i32,
}

impl From<&SystemModule> for ModuleResponse {
    fn from(m: &SystemModule) -> Self {
        Self {
            module_key: m.module_key.clone(),
            nombre: m.nombre.clone(),
            seccion: m.seccion.clone(),
            min_role: m.min_role.to_string(),
            always_visible: m.always_visible,
            orden: m.orden,
        }
    }
}

/// Response for `GET /users/{id}/modules`
#[derive(Debug, Clone, Serialize)]
pub struct UserModulesResponse {
    pub usuario_id: i32,
    /// Explicit grants for this user (only meaningful for role = usuario)
    pub granted: Vec<String>,
}

/// Request body for `PUT /users/{id}/modules` — replaces the full grant set
#[derive(Debug, Clone, Deserialize)]
pub struct ModuleGrantRequest {
    pub modules: Vec<String>,
}
