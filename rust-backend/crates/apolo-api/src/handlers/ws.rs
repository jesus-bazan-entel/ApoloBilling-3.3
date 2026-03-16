//! WebSocket handler for real-time active calls updates
//!
//! Provides real-time updates to connected clients about active calls.

use actix_web::{web, HttpRequest, HttpResponse};
use actix_ws::{Message, Session};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    /// List of all active calls
    #[serde(rename = "active_calls")]
    ActiveCalls(Vec<WsActiveCall>),
    /// A call has started
    #[serde(rename = "call_start")]
    CallStart(WsActiveCall),
    /// A call has been updated
    #[serde(rename = "call_update")]
    CallUpdate(WsActiveCall),
    /// A call has ended
    #[serde(rename = "call_end")]
    CallEnd(WsActiveCall),
    /// Dashboard stats update
    #[serde(rename = "stats_update")]
    StatsUpdate(WsDashboardStats),
    /// Error message
    #[serde(rename = "error")]
    Error { message: String },
    /// Ping/pong for keepalive
    #[serde(rename = "pong")]
    Pong,
}

/// Active call data for WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsActiveCall {
    pub call_uuid: String,
    pub account_id: Option<i32>,
    pub caller_number: String,
    pub callee_number: String,
    pub zone_name: Option<String>,
    pub direction: String,
    pub start_time: DateTime<Utc>,
    pub duration_seconds: i32,
    pub status: String,
    pub estimated_cost: Option<f64>,
}

/// Dashboard stats for WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsDashboardStats {
    pub active_calls: i64,
    pub calls_today: i64,
    pub revenue_today: f64,
}

/// Fetch active calls from database
async fn fetch_active_calls(pool: &PgPool) -> Result<Vec<WsActiveCall>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT
            call_id as call_uuid,
            calling_number,
            called_number,
            direction,
            start_time,
            current_duration,
            current_cost,
            connection_id
        FROM active_calls
        ORDER BY start_time DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let calls: Vec<WsActiveCall> = rows
        .iter()
        .map(|row| {
            let cost: Option<Decimal> = row.get("current_cost");
            WsActiveCall {
                call_uuid: row.get("call_uuid"),
                account_id: None,
                caller_number: row
                    .get::<Option<String>, _>("calling_number")
                    .unwrap_or_default(),
                callee_number: row
                    .get::<Option<String>, _>("called_number")
                    .unwrap_or_default(),
                zone_name: None,
                direction: row
                    .get::<Option<String>, _>("direction")
                    .unwrap_or_else(|| "outbound".to_string()),
                start_time: row.get("start_time"),
                duration_seconds: row.get::<Option<i32>, _>("current_duration").unwrap_or(0),
                status: "answered".to_string(),
                estimated_cost: cost.map(|c| c.to_string().parse::<f64>().unwrap_or(0.0)),
            }
        })
        .collect();

    Ok(calls)
}

/// WebSocket connection handler
pub async fn ws_handler(
    req: HttpRequest,
    body: web::Payload,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let (response, session, mut msg_stream) = actix_ws::handle(&req, body)?;

    let client_ip = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or("unknown")
        .to_string();

    info!(client = %client_ip, "WebSocket connection established");

    // Spawn WebSocket handler task
    let pool_clone = pool.get_ref().clone();
    actix_web::rt::spawn(async move {
        ws_session(session, msg_stream, pool_clone, client_ip).await;
    });

    Ok(response)
}

/// Handle WebSocket session
async fn ws_session(
    mut session: Session,
    mut msg_stream: actix_ws::MessageStream,
    pool: PgPool,
    client_ip: String,
) {
    // Send initial active calls
    if let Ok(calls) = fetch_active_calls(&pool).await {
        let msg = WsMessage::ActiveCalls(calls);
        if let Ok(json) = serde_json::to_string(&msg) {
            if session.text(json).await.is_err() {
                warn!(client = %client_ip, "Failed to send initial data, closing connection");
                return;
            }
        }
    }

    // Create interval for periodic updates (every 5 seconds)
    let mut update_interval = interval(Duration::from_secs(5));
    let mut ping_interval = interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            // Handle incoming messages
            Some(msg) = msg_stream.next() => {
                match msg {
                    Ok(Message::Text(text)) => {
                        debug!(client = %client_ip, "Received text: {}", text);
                        // Handle ping
                        if text.contains("ping") {
                            let pong = WsMessage::Pong;
                            if let Ok(json) = serde_json::to_string(&pong) {
                                let _ = session.text(json).await;
                            }
                        }
                    }
                    Ok(Message::Binary(_)) => {
                        debug!(client = %client_ip, "Received binary message");
                    }
                    Ok(Message::Ping(msg)) => {
                        if session.pong(&msg).await.is_err() {
                            break;
                        }
                    }
                    Ok(Message::Pong(_)) => {}
                    Ok(Message::Close(reason)) => {
                        info!(client = %client_ip, "Client closed connection: {:?}", reason);
                        break;
                    }
                    Ok(Message::Continuation(_)) => {}
                    Ok(Message::Nop) => {}
                    Err(e) => {
                        error!(client = %client_ip, "WebSocket error: {}", e);
                        break;
                    }
                }
            }

            // Send periodic updates
            _ = update_interval.tick() => {
                match fetch_active_calls(&pool).await {
                    Ok(calls) => {
                        let msg = WsMessage::ActiveCalls(calls);
                        if let Ok(json) = serde_json::to_string(&msg) {
                            if session.text(json).await.is_err() {
                                warn!(client = %client_ip, "Failed to send update, closing connection");
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        error!(client = %client_ip, "Database error fetching calls: {}", e);
                    }
                }
            }

            // Send periodic pings to keep connection alive
            _ = ping_interval.tick() => {
                if session.ping(b"").await.is_err() {
                    warn!(client = %client_ip, "Failed to send ping, closing connection");
                    break;
                }
            }
        }
    }

    info!(client = %client_ip, "WebSocket connection closed");
    let _ = session.close(None).await;
}
