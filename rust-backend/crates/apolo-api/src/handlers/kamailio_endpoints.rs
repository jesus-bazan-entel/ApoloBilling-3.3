//! Kamailio Endpoints (PBX) handler - Gestión directa de endpoints en Kamailio
//! Bypass de dSIPRouter API - acceso directo a la base de datos

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::{MySqlPool, Row};

/// Endpoint/PBX en Kamailio
#[derive(Debug, Serialize, Deserialize)]
pub struct KamailioEndpoint {
    pub gwid: u32,
    pub gwgroupid: u32,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_timeout: Option<u32>,
}

/// Grupo de endpoints
#[derive(Debug, Serialize, Deserialize)]
pub struct EndpointGroup {
    pub id: u32,
    pub name: String,
    pub gwlist: String,
    pub endpoint_count: i64,
}

/// Request para crear/actualizar endpoint
#[derive(Debug, Deserialize)]
pub struct CreateEndpointRequest {
    pub name: String,
    pub address: String,
    pub port: Option<u16>,
    pub description: Option<String>,
    pub call_limit: Option<u32>,
    pub call_timeout: Option<u32>,
}

/// Listar todos los endpoints (PBX) de Kamailio
async fn list_endpoints(
    kamailio_db: web::Data<MySqlPool>,
) -> actix_web::Result<HttpResponse> {
    // Query que obtiene endpoints tipo PBX (type=9)
    let query = r#"
        SELECT
            g.gwid,
            CAST(IFNULL(m.gwgroupid, 0) AS UNSIGNED) as gwgroupid,
            g.address,
            g.description as gw_description,
            IFNULL(gl.description, '') as group_description,
            CAST(IFNULL(cs.`limit`, 0) AS UNSIGNED) as call_limit,
            CAST(IFNULL(cs.timeout, 0) AS UNSIGNED) as call_timeout
        FROM dr_gateways g
        LEFT JOIN dsip_gw2gwgroup m ON g.gwid = m.gwid
        LEFT JOIN dr_gw_lists gl ON m.gwgroupid = gl.id
        LEFT JOIN dsip_call_settings cs ON m.gwgroupid = cs.gwgroupid
        WHERE g.type = 9
        ORDER BY g.gwid
    "#;

    let rows = sqlx::query(query)
        .fetch_all(kamailio_db.get_ref())
        .await
        .map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("DB error: {}", e))
        })?;

    let endpoints: Vec<KamailioEndpoint> = rows
        .iter()
        .map(|row| {
            let address: String = row.get("address");
            let (host, port) = parse_address(&address);
            let group_desc: String = row.get("group_description");
            let name = extract_name_from_description(&group_desc);

            KamailioEndpoint {
                gwid: row.get("gwid"),
                gwgroupid: row.get("gwgroupid"),
                name,
                address: host,
                port,
                description: row.get("gw_description"),
                call_limit: row.get("call_limit"),
                call_timeout: row.get("call_timeout"),
            }
        })
        .collect();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "data": endpoints,
        "count": endpoints.len()
    })))
}

/// Listar grupos de endpoints
async fn list_endpoint_groups(
    kamailio_db: web::Data<MySqlPool>,
) -> actix_web::Result<HttpResponse> {
    let query = r#"
        SELECT
            gl.id,
            gl.gwlist,
            gl.description,
            (SELECT COUNT(*) FROM dsip_gw2gwgroup m WHERE m.gwgroupid = gl.id) as endpoint_count
        FROM dr_gw_lists gl
        WHERE gl.description LIKE '%type:9%'
        ORDER BY gl.id
    "#;

    let rows = sqlx::query(query)
        .fetch_all(kamailio_db.get_ref())
        .await
        .map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("DB error: {}", e))
        })?;

    let groups: Vec<EndpointGroup> = rows
        .iter()
        .map(|row| {
            let desc: String = row.get("description");
            let name = extract_name_from_description(&desc);
            EndpointGroup {
                id: row.get("id"),
                name,
                gwlist: row.get("gwlist"),
                endpoint_count: row.get("endpoint_count"),
            }
        })
        .collect();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "data": groups,
        "count": groups.len()
    })))
}

/// Obtener un endpoint específico
async fn get_endpoint(
    kamailio_db: web::Data<MySqlPool>,
    path: web::Path<u32>,
) -> actix_web::Result<HttpResponse> {
    let gwid = path.into_inner();

    let query = r#"
        SELECT
            g.gwid,
            CAST(IFNULL(m.gwgroupid, 0) AS UNSIGNED) as gwgroupid,
            g.address,
            g.description as gw_description,
            IFNULL(gl.description, '') as group_description,
            CAST(IFNULL(cs.`limit`, 0) AS UNSIGNED) as call_limit,
            CAST(IFNULL(cs.timeout, 0) AS UNSIGNED) as call_timeout
        FROM dr_gateways g
        LEFT JOIN dsip_gw2gwgroup m ON g.gwid = m.gwid
        LEFT JOIN dr_gw_lists gl ON m.gwgroupid = gl.id
        LEFT JOIN dsip_call_settings cs ON m.gwgroupid = cs.gwgroupid
        WHERE g.gwid = ? AND g.type = 9
    "#;

    let row = sqlx::query(query)
        .bind(gwid)
        .fetch_optional(kamailio_db.get_ref())
        .await
        .map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("DB error: {}", e))
        })?;

    match row {
        Some(row) => {
            let address: String = row.get("address");
            let (host, port) = parse_address(&address);
            let group_desc: String = row.get("group_description");
            let name = extract_name_from_description(&group_desc);

            let endpoint = KamailioEndpoint {
                gwid: row.get("gwid"),
                gwgroupid: row.get("gwgroupid"),
                name,
                address: host,
                port,
                description: row.get("gw_description"),
                call_limit: row.get("call_limit"),
                call_timeout: row.get("call_timeout"),
            };

            Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "data": endpoint
            })))
        }
        None => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "success": false,
            "error": "Endpoint not found"
        }))),
    }
}

/// Crear un nuevo endpoint
async fn create_endpoint(
    kamailio_db: web::Data<MySqlPool>,
    body: web::Json<CreateEndpointRequest>,
) -> actix_web::Result<HttpResponse> {
    let port = body.port.unwrap_or(5060);
    let address = format!("{}:{}", body.address, port);
    let description = body.description.clone().unwrap_or_else(|| body.name.clone());

    // Iniciar transacción
    let mut tx = kamailio_db.begin().await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Transaction error: {}", e))
    })?;

    // 1. Insertar gateway
    let result = sqlx::query(
        r#"INSERT INTO dr_gateways (type, address, strip, pri_prefix, attrs, description)
           VALUES (9, ?, 0, '', '', ?)"#
    )
    .bind(&address)
    .bind(&description)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Insert gateway error: {}", e))
    })?;

    let gwid = result.last_insert_id();

    // 2. Actualizar attrs con gwid,type
    sqlx::query("UPDATE dr_gateways SET attrs = ? WHERE gwid = ?")
        .bind(format!("{},9", gwid))
        .bind(gwid)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("Update attrs error: {}", e))
        })?;

    // 3. Crear grupo
    let group_desc = format!("name:{},type:9", body.name);
    sqlx::query(
        r#"INSERT INTO dr_gw_lists (id, gwlist, description) VALUES (?, ?, ?)"#
    )
    .bind(gwid)
    .bind(gwid.to_string())
    .bind(&group_desc)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Insert group error: {}", e))
    })?;

    // 4. Mapping gw2gwgroup
    sqlx::query(
        r#"INSERT INTO dsip_gw2gwgroup (gwid, gwgroupid, key_type, value_type) VALUES (?, ?, 0, 0)"#
    )
    .bind(gwid)
    .bind(gwid)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Insert mapping error: {}", e))
    })?;

    // 5. ACL en address
    let acl_tag = format!("name:{},gwgroup:{}", body.name, gwid);
    sqlx::query(
        r#"INSERT INTO address (grp, ip_addr, mask, port, tag) VALUES (9, ?, 32, ?, ?)"#
    )
    .bind(&body.address)
    .bind(port)
    .bind(&acl_tag)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Insert ACL error: {}", e))
    })?;

    // 6. Call settings
    sqlx::query(
        r#"INSERT INTO dsip_call_settings (gwgroupid, `limit`, timeout) VALUES (?, ?, ?)"#
    )
    .bind(gwid)
    .bind(body.call_limit.unwrap_or(0))
    .bind(body.call_timeout.unwrap_or(0))
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Insert call settings error: {}", e))
    })?;

    // Commit
    tx.commit().await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Commit error: {}", e))
    })?;

    // Recargar Kamailio
    reload_kamailio().await;

    Ok(HttpResponse::Created().json(serde_json::json!({
        "success": true,
        "data": {
            "gwid": gwid,
            "gwgroupid": gwid,
            "name": body.name,
            "address": body.address,
            "port": port
        },
        "message": "Endpoint created successfully"
    })))
}

/// Eliminar un endpoint
async fn delete_endpoint(
    kamailio_db: web::Data<MySqlPool>,
    path: web::Path<u32>,
) -> actix_web::Result<HttpResponse> {
    let gwid = path.into_inner();

    let mut tx = kamailio_db.begin().await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Transaction error: {}", e))
    })?;

    // Eliminar en orden inverso de dependencias
    sqlx::query("DELETE FROM dsip_call_settings WHERE gwgroupid = ?")
        .bind(gwid)
        .execute(&mut *tx)
        .await
        .ok();

    sqlx::query("DELETE FROM address WHERE tag LIKE ?")
        .bind(format!("%gwgroup:{}%", gwid))
        .execute(&mut *tx)
        .await
        .ok();

    sqlx::query("DELETE FROM dsip_gw2gwgroup WHERE gwid = ?")
        .bind(gwid)
        .execute(&mut *tx)
        .await
        .ok();

    sqlx::query("DELETE FROM dr_gw_lists WHERE id = ?")
        .bind(gwid)
        .execute(&mut *tx)
        .await
        .ok();

    sqlx::query("DELETE FROM dr_gateways WHERE gwid = ? AND type = 9")
        .bind(gwid)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("Delete error: {}", e))
        })?;

    tx.commit().await.map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Commit error: {}", e))
    })?;

    reload_kamailio().await;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "Endpoint deleted successfully"
    })))
}

// Helpers
fn parse_address(addr: &str) -> (String, u16) {
    if let Some(idx) = addr.rfind(':') {
        let host = addr[..idx].to_string();
        let port = addr[idx + 1..].parse().unwrap_or(5060);
        (host, port)
    } else {
        (addr.to_string(), 5060)
    }
}

fn extract_name_from_description(desc: &str) -> String {
    for part in desc.split(',') {
        if part.starts_with("name:") {
            return part[5..].to_string();
        }
    }
    desc.to_string()
}

async fn reload_kamailio() {
    // Ejecutar kamcmd para recargar configuración
    let _ = tokio::process::Command::new("kamcmd")
        .args(["drouting.reload"])
        .output()
        .await;
    let _ = tokio::process::Command::new("kamcmd")
        .args(["permissions.addressReload"])
        .output()
        .await;
    let _ = tokio::process::Command::new("kamcmd")
        .args(["htable.reload", "gw2gwgroup"])
        .output()
        .await;
}

/// Configurar rutas
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/kamailio-endpoints")
            .route("", web::get().to(list_endpoints))
            .route("/groups", web::get().to(list_endpoint_groups))
            .route("/{gwid}", web::get().to(get_endpoint))
            .route("", web::post().to(create_endpoint))
            .route("/{gwid}", web::delete().to(delete_endpoint))
    );
}
