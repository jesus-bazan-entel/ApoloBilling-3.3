// src/esl/connection.rs
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use crate::esl::event::EslEvent;

pub struct EslConnection {
    stream: Arc<Mutex<BufReader<TcpStream>>>,
    server_id: String,
}

impl EslConnection {
    pub async fn connect(
        host: &str,
        port: u16,
        password: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let server_id = format!("{}:{}", host, port);
        info!("Connecting to FreeSWITCH ESL: {}", server_id);

        // Connect TCP
        let stream = TcpStream::connect(format!("{}:{}", host, port)).await?;
        let mut reader = BufReader::new(stream);

        // Read greeting
        let greeting = Self::read_response(&mut reader).await?;
        
        if !greeting.contains("Content-Type: auth/request") {
            return Err("Invalid greeting from FreeSWITCH".into());
        }

        debug!("Received auth request from {}", server_id);

        // Authenticate
        Self::write_command(&mut reader, &format!("auth {}\n\n", password)).await?;
        
        let auth_response = Self::read_response(&mut reader).await?;
        
        if !auth_response.contains("Reply-Text: +OK accepted") {
            return Err("Authentication failed".into());
        }

        info!("✅ Authenticated to FreeSWITCH: {}", server_id);

        // Subscribe to events
        Self::write_command(
            &mut reader,
            "event plain CHANNEL_CREATE CHANNEL_ANSWER CHANNEL_HANGUP_COMPLETE\n\n"
        ).await?;

        let event_response = Self::read_response(&mut reader).await?;
        
        if !event_response.contains("Reply-Text: +OK") {
            return Err("Failed to subscribe to events".into());
        }

        info!("✅ Subscribed to events: {}", server_id);

        Ok(Self {
            stream: Arc::new(Mutex::new(reader)),
            server_id,
        })
    }

    pub async fn read_event(&self) -> Result<Option<EslEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let mut stream = self.stream.lock().await;
        let data = Self::read_response(&mut *stream).await?;

        if data.contains("Event-Name:") {
            Ok(EslEvent::parse(&data))
        } else {
            Ok(None)
        }
    }

    pub async fn send_command(&self, command: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut stream = self.stream.lock().await;
        Self::write_command(&mut *stream, command).await?;

        // Keep reading until we get a command response (not an event)
        // Events have "Content-Type: text/event-plain"
        // Command responses have "Content-Type: api/response" or "Content-Type: command/reply"
        loop {
            let response = Self::read_response(&mut *stream).await?;

            // Check if this is a command response (not an event)
            if response.contains("Content-Type: api/response")
                || response.contains("Content-Type: command/reply")
            {
                return Ok(response);
            }

            // If it's an event, log and continue reading
            if response.contains("Content-Type: text/event-plain") {
                debug!("Skipping event while waiting for command response");
                continue;
            }

            // Unknown response type, return it anyway
            return Ok(response);
        }
    }

    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    // Helper: Read response from FreeSWITCH
    async fn read_response(
        reader: &mut BufReader<TcpStream>
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut response = String::new();
        let mut content_length: Option<usize> = None;
        
        // Read headers
        loop {
            let mut line = String::new();
            let mut buf = [0u8; 1];
            
            // Read line byte by byte (until \n)
            loop {
                reader.read_exact(&mut buf).await?;
                if buf[0] == b'\n' {
                    break;
                }
                line.push(buf[0] as char);
            }

            // Remove trailing \r
            if line.ends_with('\r') {
                line.pop();
            }

            response.push_str(&line);
            response.push('\n');

            // Check for Content-Length header
            if line.starts_with("Content-Length:") {
                if let Some(len_str) = line.split(':').nth(1) {
                    content_length = len_str.trim().parse().ok();
                }
            }

            // Empty line = end of headers
            if line.is_empty() {
                break;
            }
        }

        // Read body if Content-Length present
        if let Some(len) = content_length {
            let mut body = vec![0u8; len];
            reader.read_exact(&mut body).await?;
            response.push_str(&String::from_utf8_lossy(&body));
        }

        Ok(response)
    }

    // Helper: Write command to FreeSWITCH
    async fn write_command(
        reader: &mut BufReader<TcpStream>,
        command: &str
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let stream = reader.get_mut();
        stream.write_all(command.as_bytes()).await?;
        stream.flush().await?;
        Ok(())
    }
}
