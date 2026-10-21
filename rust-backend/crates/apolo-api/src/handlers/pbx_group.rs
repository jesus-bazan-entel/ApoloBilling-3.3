//! PBX Group handlers (same-owner failover sets for IP trunks)
//!
//! Mirror of Carrier Groups, but for customer PBXs. L3 routes each owned
//! prefix to the group members in priority order — legitimate same-owner
//! failover, never cross-customer.

use crate::dto::pbx_group::*;
use crate::dto::ApiResponse;
use actix_web::{web, HttpResponse};
use apolo_auth::AdminUser;
use apolo_core::AppError;
use sqlx::PgPool;
use tracing::{info, instrument};
use uuid::Uuid;
use validator::Validate;

/// All trunk/ip endpoints' ranges, except members of `exclude_group`.
async fn ranges_outside_group(
    pool: &PgPool,
    exclude_group: Option<Uuid>,
) -> Result<Vec<(String, Vec<String>)>, AppError> {
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        r#"SELECT se.name, se.did_ranges FROM sip_endpoints se
            WHERE se.enabled = true AND se.endpoint_type = 'trunk' AND auth_mode = 'ip'
              AND COALESCE(se.did_ranges,'') <> ''
              AND ($1::uuid IS NULL OR se.id NOT IN
                   (SELECT sip_endpoint_id FROM pbx_group_members WHERE group_id = $1))"#,
    )
    .bind(exclude_group)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Database(format!("Failed to read DID ranges: {}", e)))?;
    Ok(rows
        .into_iter()
        .map(|(name, r)| {
            let prefixes = r
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(|p| p.to_string())
                .collect();
            (name, prefixes)
        })
        .collect())
}

/// List all PBX groups with members (ordered by priority)
#[instrument(skip(pool, _admin))]
pub async fn list_pbx_groups(
    pool: web::Data<PgPool>,
    _admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let groups: Vec<(Uuid, String, Option<String>, String, bool, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT id, name, description, strategy, enabled, created_at, updated_at FROM pbx_groups ORDER BY name",
        )
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to list PBX groups: {}", e)))?;

    let mut out = Vec::new();
    for (id, name, description, strategy, enabled, created_at, updated_at) in groups {
        let members: Vec<(Uuid, Uuid, String, String, i32, i32)> = sqlx::query_as(
            r#"SELECT m.id, m.sip_endpoint_id, se.name, se.host, se.port, m.priority
                FROM pbx_group_members m JOIN sip_endpoints se ON se.id = m.sip_endpoint_id
                WHERE m.group_id = $1 ORDER BY m.priority, se.name"#,
        )
        .bind(id)
        .fetch_all(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to list group members: {}", e)))?;
        out.push(PbxGroupWithMembers {
            id,
            name,
            description,
            strategy,
            enabled,
            created_at,
            updated_at,
            members: members
                .into_iter()
                .map(|(mid, eid, ename, host, port, prio)| PbxGroupMember {
                    id: mid,
                    sip_endpoint_id: eid,
                    endpoint_name: ename,
                    host,
                    port,
                    priority: prio,
                })
                .collect(),
        });
    }
    Ok(HttpResponse::Ok().json(ApiResponse::success(out)))
}

/// Create a PBX group
#[instrument(skip(pool, admin, req))]
pub async fn create_pbx_group(
    pool: web::Data<PgPool>,
    admin: AdminUser,
    req: web::Json<CreatePbxGroupRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    let strategy = req
        .strategy
        .clone()
        .unwrap_or_else(|| "sequential".to_string());
    if strategy != "sequential" && strategy != "priority" {
        return Err(AppError::Validation(
            "strategy must be sequential|priority".to_string(),
        ));
    }
    info!(name = %req.name, "Creating PBX group");
    let row: (
        Uuid,
        String,
        Option<String>,
        String,
        bool,
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
    ) = sqlx::query_as(
        r#"INSERT INTO pbx_groups (name, description, strategy, enabled)
                VALUES ($1, $2, $3, $4)
                RETURNING id, name, description, strategy, enabled, created_at, updated_at"#,
    )
    .bind(&req.name)
    .bind(&req.description)
    .bind(&strategy)
    .bind(req.enabled.unwrap_or(true))
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        if e.to_string().contains("pbx_groups_name_key") {
            AppError::AlreadyExists(format!("PBX group '{}' already exists", req.name))
        } else {
            AppError::Database(format!("Failed to create PBX group: {}", e))
        }
    })?;
    let _ = admin.username;
    Ok(
        HttpResponse::Created().json(ApiResponse::success(PbxGroupWithMembers {
            id: row.0,
            name: row.1,
            description: row.2,
            strategy: row.3,
            enabled: row.4,
            created_at: row.5,
            updated_at: row.6,
            members: vec![],
        })),
    )
}

/// Update a PBX group
#[instrument(skip(pool, _admin, req))]
pub async fn update_pbx_group(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    _admin: AdminUser,
    req: web::Json<UpdatePbxGroupRequest>,
) -> Result<HttpResponse, AppError> {
    req.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    let id = path.into_inner();
    if let Some(ref s) = req.strategy {
        if s != "sequential" && s != "priority" {
            return Err(AppError::Validation(
                "strategy must be sequential|priority".to_string(),
            ));
        }
    }
    let row: Option<(
        Uuid,
        String,
        Option<String>,
        String,
        bool,
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        r#"UPDATE pbx_groups SET
                name = COALESCE($2, name), description = COALESCE($3, description),
                strategy = COALESCE($4, strategy), enabled = COALESCE($5, enabled),
                updated_at = NOW()
               WHERE id = $1
               RETURNING id, name, description, strategy, enabled, created_at, updated_at"#,
    )
    .bind(id)
    .bind(&req.name)
    .bind(&req.description)
    .bind(&req.strategy)
    .bind(req.enabled)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to update PBX group: {}", e)))?;
    match row {
        Some(r) => Ok(
            HttpResponse::Ok().json(ApiResponse::success(PbxGroupWithMembers {
                id: r.0,
                name: r.1,
                description: r.2,
                strategy: r.3,
                enabled: r.4,
                created_at: r.5,
                updated_at: r.6,
                members: vec![],
            })),
        ),
        None => Err(AppError::NotFound(format!("PBX group {} not found", id))),
    }
}

/// Delete a PBX group (members cascade; endpoints keep their did_ranges)
#[instrument(skip(pool, _admin))]
pub async fn delete_pbx_group(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    _admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let res = sqlx::query("DELETE FROM pbx_groups WHERE id = $1")
        .bind(path.into_inner())
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to delete PBX group: {}", e)))?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("PBX group not found".to_string()));
    }
    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({"deleted": true}))))
}

/// Add an IP trunk to a group (same owner). Rejects ranges overlapping
/// endpoints OUTSIDE this group, naming the current owner.
#[instrument(skip(pool, _admin, req))]
pub async fn add_pbx_group_member(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    _admin: AdminUser,
    req: web::Json<AddPbxGroupMemberRequest>,
) -> Result<HttpResponse, AppError> {
    let group_id = path.into_inner();
    let ep: Option<(Uuid, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, name, endpoint_type, auth_mode, did_ranges FROM sip_endpoints WHERE id = $1",
    )
    .bind(req.sip_endpoint_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to read endpoint: {}", e)))?;
    let (_, ename, etype, amode, ranges) = match ep {
        Some(e) => e,
        None => return Err(AppError::NotFound("SIP endpoint not found".to_string())),
    };
    if etype != "trunk" || amode != "ip" {
        return Err(AppError::Validation(
            "Solo troncales IP (endpoint_type=trunk, auth_mode=ip) pueden agruparse".to_string(),
        ));
    }
    // Overlap vs outside the group (inside = same owner, harmless)
    let new_prefixes: Vec<String> = ranges
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect();
    if !new_prefixes.is_empty() {
        let outside = ranges_outside_group(pool.get_ref(), Some(group_id)).await?;
        let mut conflicts = Vec::new();
        for p in &new_prefixes {
            for (owner, qs) in &outside {
                for q in qs {
                    if p == q || p.starts_with(q.as_str()) || q.starts_with(p.as_str()) {
                        conflicts.push(format!("'{}' se solapa con '{}' de '{}'", p, q, owner));
                    }
                }
            }
        }
        if !conflicts.is_empty() {
            return Err(AppError::Validation(format!(
                "La PBX '{}' no puede entrar al grupo: {}. Los rangos de un grupo pertenecen a un solo dueño.",
                ename,
                conflicts.join("; ")
            )));
        }
    }
    let priority = req.priority.unwrap_or(100);
    sqlx::query(
        "INSERT INTO pbx_group_members (group_id, sip_endpoint_id, priority) VALUES ($1, $2, $3)",
    )
    .bind(group_id)
    .bind(req.sip_endpoint_id)
    .bind(priority)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("uq_pbx_group_member_endpoint") {
            AppError::Validation(
                "Esa PBX ya pertenece a otro grupo (una PBX, un grupo)".to_string(),
            )
        } else if msg.contains("uq_pbx_group_member") {
            AppError::AlreadyExists("Esa PBX ya es miembro del grupo".to_string())
        } else if msg.contains("violates foreign key") {
            AppError::NotFound("PBX group not found".to_string())
        } else {
            AppError::Database(format!("Failed to add group member: {}", e))
        }
    })?;
    info!(group = %group_id, endpoint = %ename, "PBX group member added");
    Ok(HttpResponse::Created().json(ApiResponse::success(serde_json::json!({"added": true}))))
}

/// Update a member's priority
#[instrument(skip(pool, _admin, req))]
pub async fn update_pbx_group_member(
    pool: web::Data<PgPool>,
    path: web::Path<(Uuid, Uuid)>,
    _admin: AdminUser,
    req: web::Json<UpdatePbxGroupMemberRequest>,
) -> Result<HttpResponse, AppError> {
    let (group_id, member_id) = path.into_inner();
    let res = sqlx::query(
        "UPDATE pbx_group_members SET priority = COALESCE($3, priority) WHERE id = $1 AND group_id = $2",
    )
    .bind(member_id)
    .bind(group_id)
    .bind(req.priority)
    .execute(pool.get_ref())
    .await
    .map_err(|e| AppError::Database(format!("Failed to update member: {}", e)))?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("Group member not found".to_string()));
    }
    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({"updated": true}))))
}

/// Remove a member from a group
#[instrument(skip(pool, _admin))]
pub async fn remove_pbx_group_member(
    pool: web::Data<PgPool>,
    path: web::Path<(Uuid, Uuid)>,
    _admin: AdminUser,
) -> Result<HttpResponse, AppError> {
    let (group_id, member_id) = path.into_inner();
    let res = sqlx::query("DELETE FROM pbx_group_members WHERE id = $1 AND group_id = $2")
        .bind(member_id)
        .bind(group_id)
        .execute(pool.get_ref())
        .await
        .map_err(|e| AppError::Database(format!("Failed to remove member: {}", e)))?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("Group member not found".to_string()));
    }
    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({"removed": true}))))
}

// ============================================================================
// Route Configuration
// ============================================================================

/// Configure PBX group routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/pbx-groups")
            .route("", web::get().to(list_pbx_groups))
            .route("", web::post().to(create_pbx_group))
            .route("/{id}", web::put().to(update_pbx_group))
            .route("/{id}", web::delete().to(delete_pbx_group))
            .route("/{id}/members", web::post().to(add_pbx_group_member))
            .route(
                "/{id}/members/{member_id}",
                web::put().to(update_pbx_group_member),
            )
            .route(
                "/{id}/members/{member_id}",
                web::delete().to(remove_pbx_group_member),
            ),
    );
}
