//! FreeSWITCH Event Socket Layer (ESL) integration for ApoloBilling
//!
//! This crate provides comprehensive ESL client and server functionality for
//! real-time call control and billing integration with FreeSWITCH.
//!
//! # Features
//!
//! - TCP connection management with authentication
//! - Event subscription and parsing
//! - Command execution and response handling
//! - Automatic reconnection with exponential backoff
//! - Server mode for testing and simulation
//! - Event handler integration with billing services
//!
//! # Architecture
//!
//! ```text
//! FreeSWITCH ESL Server
//!         |
//!         v
//!  EslConnection (TCP)
//!         |
//!         v
//!    EslEvent (Parser)
//!         |
//!         v
//!  EslEventHandler (Business Logic)
//!         |
//!         v
//!  Billing Services (Auth, Reservation, CDR)
//! ```
//!
//! # Example Usage
//!
//! ```rust,ignore
//! use apolo_esl::{EslClient, EslConnection};
//! use apolo_core::config::FreeSwitchServer;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let servers = vec![
//!         FreeSwitchServer {
//!             host: "localhost".to_string(),
//!             port: 8021,
//!             password: "ClueCon".to_string(),
//!             id: Some("fs1".to_string()),
//!         }
//!     ];
//!
//!     let client = EslClient::new(servers);
//!     client.run().await?;
//!
//!     Ok(())
//! }
//! ```

// NOTE: Some modules are commented out as their files don't exist yet
// pub mod client;
// pub mod connection;
pub mod event;
// pub mod event_handler;
// pub mod server;

// pub use client::EslClient;
// pub use connection::EslConnection;
pub use event::EslEvent;
// pub use event_handler::EslEventHandler;
// pub use server::EslServer;

/// ESL protocol constants
pub mod constants {
    /// Default ESL password (FreeSWITCH default)
    pub const DEFAULT_PASSWORD: &str = "ClueCon";

    /// Authentication command
    pub const AUTH_COMMAND: &str = "auth";

    /// Event subscription command
    pub const EVENT_COMMAND: &str = "event";

    /// API command prefix
    pub const API_COMMAND: &str = "api";

    /// Background API command prefix
    pub const BGAPI_COMMAND: &str = "bgapi";

    /// Command/Reply content type
    pub const CONTENT_TYPE_REPLY: &str = "command/reply";

    /// Event plain content type
    pub const CONTENT_TYPE_EVENT: &str = "text/event-plain";

    /// Authentication request content type
    pub const CONTENT_TYPE_AUTH: &str = "auth/request";

    /// Maximum reconnection attempts before giving up
    pub const MAX_RECONNECT_ATTEMPTS: u32 = 10;

    /// Initial reconnection delay in milliseconds
    pub const INITIAL_RECONNECT_DELAY_MS: u64 = 1000;

    /// Maximum reconnection delay in milliseconds
    pub const MAX_RECONNECT_DELAY_MS: u64 = 60000;

    /// Read buffer size for TCP socket
    pub const READ_BUFFER_SIZE: usize = 8192;

    /// Command timeout in seconds
    pub const COMMAND_TIMEOUT_SECS: u64 = 10;
}

/// ESL events we subscribe to
pub mod events {
    /// Channel created (call initiated)
    pub const CHANNEL_CREATE: &str = "CHANNEL_CREATE";

    /// Channel answered (call connected)
    pub const CHANNEL_ANSWER: &str = "CHANNEL_ANSWER";

    /// Channel hangup complete (call ended)
    pub const CHANNEL_HANGUP_COMPLETE: &str = "CHANNEL_HANGUP_COMPLETE";

    /// Channel state change
    pub const CHANNEL_STATE: &str = "CHANNEL_STATE";

    /// Channel bridge (two channels connected)
    pub const CHANNEL_BRIDGE: &str = "CHANNEL_BRIDGE";

    /// Channel unbridge (channels disconnected)
    pub const CHANNEL_UNBRIDGE: &str = "CHANNEL_UNBRIDGE";

    /// Heartbeat event
    pub const HEARTBEAT: &str = "HEARTBEAT";

    /// All events we subscribe to for billing
    pub const BILLING_EVENTS: &[&str] = &[
        CHANNEL_CREATE,
        CHANNEL_ANSWER,
        CHANNEL_HANGUP_COMPLETE,
        CHANNEL_STATE,
        CHANNEL_BRIDGE,
        CHANNEL_UNBRIDGE,
        HEARTBEAT,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(constants::DEFAULT_PASSWORD, "ClueCon");
        assert_eq!(constants::AUTH_COMMAND, "auth");
        assert!(constants::MAX_RECONNECT_DELAY_MS > constants::INITIAL_RECONNECT_DELAY_MS);
    }

    #[test]
    fn test_billing_events() {
        assert!(events::BILLING_EVENTS.contains(&events::CHANNEL_CREATE));
        assert!(events::BILLING_EVENTS.contains(&events::CHANNEL_ANSWER));
        assert!(events::BILLING_EVENTS.contains(&events::CHANNEL_HANGUP_COMPLETE));
    }
}
