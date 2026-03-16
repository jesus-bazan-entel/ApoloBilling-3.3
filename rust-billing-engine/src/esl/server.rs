// src/esl/server.rs
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::Arc;
use tracing::{info, error, warn};

use crate::services::{AuthorizationService, RealtimeBiller, CdrGenerator};
use crate::database::DbPool;
use crate::cache::RedisClient;
use super::event_handler::EventHandler;

pub struct EslServer {
    auth_service: Arc<AuthorizationService>,
    realtime_biller: Arc<RealtimeBiller>,
    cdr_generator: Arc<CdrGenerator>,
    db_pool: DbPool,
    redis_client: RedisClient,
}

impl EslServer {
    pub fn new(
        auth_service: Arc<AuthorizationService>,
        realtime_biller: Arc<RealtimeBiller>,
        cdr_generator: Arc<CdrGenerator>,
        db_pool: DbPool,
        redis_client: RedisClient,
    ) -> Self {
        Self {
            auth_service,
            realtime_biller,
            cdr_generator,
            db_pool,
            redis_client,
        }
    }

    pub async fn start(&self, bind_address: &str) -> std::io::Result<()> {
        let listener = TcpListener::bind(bind_address).await?;
        info!("🎧 ESL Server listening on {}", bind_address);

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    info!("ESL connection accepted from {}", addr);
                    
                    let handler = EventHandler::new_server_mode(
                        self.auth_service.clone(),
                        self.realtime_biller.clone(),
                        self.cdr_generator.clone(),
                        self.db_pool.clone(),
                        self.redis_client.clone(),
                    );

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(socket, handler).await {
                            error!("Error handling ESL connection from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept ESL connection: {}", e);
                }
            }
        }
    }

    async fn handle_connection(
        mut socket: TcpStream,
        handler: EventHandler,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Send ESL auth request
        socket.write_all(b"Content-Type: auth/request\n\n").await?;

        let mut reader = BufReader::new(socket);
        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await?;
            
            if bytes_read == 0 {
                // Connection closed
                break;
            }

            let line_trimmed = line.trim();

            // Handle auth command
            if line_trimmed.starts_with("auth ") {
                let socket = reader.get_mut();
                socket.write_all(b"Content-Type: command/reply\nReply-Text: +OK accepted\n\n").await?;
                info!("ESL client authenticated");
                continue;
            }

            // Handle event subscription
            if line_trimmed.starts_with("event ") || line_trimmed == "myevents" {
                let socket = reader.get_mut();
                socket.write_all(b"Content-Type: command/reply\nReply-Text: +OK event listener enabled\n\n").await?;
                info!("ESL client subscribed to events");
                continue;
            }

            // Handle ESL events (from simulator)
            if line_trimmed.starts_with("Event-Name: ") {
                let event_name = line_trimmed.replace("Event-Name: ", "");
                
                // Read event headers until blank line
                let mut headers = vec![line_trimmed.to_string()];
                loop {
                    line.clear();
                    let bytes = reader.read_line(&mut line).await?;
                    if bytes == 0 || line.trim().is_empty() {
                        break;
                    }
                    headers.push(line.trim().to_string());
                }

                // Parse event
                let event = Self::parse_event(&event_name, &headers);
                
                // Process event with handler
                if let Err(e) = handler.handle_event(&event).await {
                    error!("Error processing event {}: {}", event_name, e);
                }

                continue;
            }

            // Handle sendevent command (from simulator)
            if line_trimmed.starts_with("sendevent ") {
                let event_name = line_trimmed.replace("sendevent ", "");
                
                // Read event headers until blank line
                let mut headers = Vec::new();
                loop {
                    line.clear();
                    let bytes = reader.read_line(&mut line).await?;
                    if bytes == 0 || line.trim().is_empty() {
                        break;
                    }
                    headers.push(line.trim().to_string());
                }

                // Parse event
                let event = Self::parse_event(&event_name, &headers);
                
                // Process event with handler
                match handler.handle_event(&event).await {
                    Ok(_) => {
                        let socket = reader.get_mut();
                        socket.write_all(b"Content-Type: command/reply\nReply-Text: +OK\n\n").await?;
                    }
                    Err(e) => {
                        error!("Error processing event {}: {}", event_name, e);
                        let socket = reader.get_mut();
                        socket.write_all(b"Content-Type: command/reply\nReply-Text: -ERR\n\n").await?;
                    }
                }

                continue;
            }

            // Ignore empty lines
            if line_trimmed.is_empty() {
                continue;
            }

            // Unknown command - send OK to keep connection alive
            warn!("Unknown ESL command: {}", line_trimmed);
        }

        info!("ESL connection closed");
        Ok(())
    }

    fn parse_event(event_name: &str, headers: &[String]) -> super::event::EslEvent {
        use super::event::EslEvent;
        use std::collections::HashMap;

        let mut event_headers = HashMap::new();

        // Always include Event-Name
        event_headers.insert("Event-Name".to_string(), event_name.to_string());

        // Parse all headers
        for header in headers {
            if let Some(pos) = header.find(':') {
                let key = header[..pos].trim().to_string();
                let value = header[pos + 1..].trim().to_string();
                event_headers.insert(key, value);
            }
        }

        EslEvent {
            headers: event_headers,
            body: None,
        }
    }
}
