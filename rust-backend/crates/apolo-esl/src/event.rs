//! ESL event parsing and representation
//!
//! This module handles parsing FreeSWITCH event messages and provides
//! convenient access to event headers and variables.

use std::collections::HashMap;
use std::fmt;

/// ESL Event structure
///
/// Represents a parsed FreeSWITCH event with headers as key-value pairs.
/// Events use URL-encoded format for headers.
#[derive(Debug, Clone, Default)]
pub struct EslEvent {
    /// Event headers (key-value pairs)
    headers: HashMap<String, String>,

    /// Raw event body (if present)
    body: Option<String>,
}

impl EslEvent {
    /// Create a new empty event
    pub fn new() -> Self {
        Self {
            headers: HashMap::new(),
            body: None,
        }
    }

    /// Parse an ESL event from raw text
    ///
    /// ESL events are formatted as:
    /// ```text
    /// Header-Name: value
    /// Another-Header: another value
    ///
    /// Optional body content
    /// ```
    pub fn parse(raw: &str) -> Self {
        let mut headers = HashMap::new();
        let mut body = None;
        let mut lines = raw.lines();
        let mut in_body = false;
        let mut body_lines = Vec::new();

        for line in lines {
            if in_body {
                body_lines.push(line);
                continue;
            }

            // Empty line separates headers from body
            if line.trim().is_empty() {
                in_body = true;
                continue;
            }

            // Parse header line: "Key: Value"
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim().to_string();
                let value = value.trim().to_string();

                // URL decode the value
                let decoded_value = urlencoding::decode(&value)
                    .unwrap_or_else(|_| value.clone().into())
                    .to_string();

                headers.insert(key, decoded_value);
            }
        }

        if !body_lines.is_empty() {
            body = Some(body_lines.join("\n"));
        }

        Self { headers, body }
    }

    /// Get a header value by name
    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(|s| s.as_str())
    }

    /// Get a header value or default
    pub fn get_header_or(&self, name: &str, default: &str) -> String {
        self.get_header(name)
            .unwrap_or(default)
            .to_string()
    }

    /// Get all headers
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    /// Get event body
    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    /// Set a header
    pub fn set_header(&mut self, key: String, value: String) {
        self.headers.insert(key, value);
    }

    /// Set the body
    pub fn set_body(&mut self, body: String) {
        self.body = Some(body);
    }

    // Common event headers with convenient accessors

    /// Get event name (Event-Name header)
    pub fn event_name(&self) -> Option<&str> {
        self.get_header("Event-Name")
    }

    /// Get unique ID (Unique-ID header)
    pub fn unique_id(&self) -> Option<&str> {
        self.get_header("Unique-ID")
            .or_else(|| self.get_header("Channel-Call-UUID"))
    }

    /// Get caller ANI/CLI (Caller-ANI header)
    pub fn caller_number(&self) -> Option<&str> {
        self.get_header("Caller-ANI")
            .or_else(|| self.get_header("Caller-Caller-ID-Number"))
            .or_else(|| self.get_header("variable_sip_from_user"))
    }

    /// Get destination number (Caller-Destination-Number header)
    pub fn destination_number(&self) -> Option<&str> {
        self.get_header("Caller-Destination-Number")
            .or_else(|| self.get_header("variable_sip_to_user"))
    }

    /// Get channel call UUID
    pub fn call_uuid(&self) -> Option<&str> {
        self.get_header("Channel-Call-UUID")
            .or_else(|| self.get_header("Unique-ID"))
    }

    /// Get hangup cause
    pub fn hangup_cause(&self) -> Option<&str> {
        self.get_header("Hangup-Cause")
            .or_else(|| self.get_header("variable_hangup_cause"))
    }

    /// Get billsec (billable seconds)
    pub fn billsec(&self) -> Option<i32> {
        self.get_header("variable_billsec")
            .or_else(|| self.get_header("Billsec"))
            .and_then(|s| s.parse().ok())
    }

    /// Get duration (total call duration)
    pub fn duration(&self) -> Option<i32> {
        self.get_header("variable_duration")
            .or_else(|| self.get_header("Duration"))
            .and_then(|s| s.parse().ok())
    }

    /// Get answer time epoch
    pub fn answer_epoch(&self) -> Option<i64> {
        self.get_header("Caller-Channel-Answered-Time")
            .or_else(|| self.get_header("variable_answer_epoch"))
            .and_then(|s| s.parse().ok())
    }

    /// Get start time epoch
    pub fn start_epoch(&self) -> Option<i64> {
        self.get_header("Caller-Channel-Created-Time")
            .or_else(|| self.get_header("variable_start_epoch"))
            .and_then(|s| s.parse().ok())
    }

    /// Get end time epoch
    pub fn end_epoch(&self) -> Option<i64> {
        self.get_header("Caller-Channel-Hangup-Time")
            .or_else(|| self.get_header("variable_end_epoch"))
            .and_then(|s| s.parse().ok())
    }

    /// Get channel state
    pub fn channel_state(&self) -> Option<&str> {
        self.get_header("Channel-State")
    }

    /// Get answer state
    pub fn answer_state(&self) -> Option<&str> {
        self.get_header("Answer-State")
    }

    /// Get call direction
    pub fn direction(&self) -> Option<&str> {
        self.get_header("Call-Direction")
            .or_else(|| self.get_header("variable_direction"))
    }

    /// Get content type
    pub fn content_type(&self) -> Option<&str> {
        self.get_header("Content-Type")
    }

    /// Get content length
    pub fn content_length(&self) -> Option<usize> {
        self.get_header("Content-Length")
            .and_then(|s| s.parse().ok())
    }

    /// Get reply text
    pub fn reply_text(&self) -> Option<&str> {
        self.get_header("Reply-Text")
    }

    /// Check if this is a command reply
    pub fn is_command_reply(&self) -> bool {
        self.content_type() == Some("command/reply")
    }

    /// Check if this is an auth request
    pub fn is_auth_request(&self) -> bool {
        self.content_type() == Some("auth/request")
    }

    /// Check if this is an event
    pub fn is_event(&self) -> bool {
        self.content_type() == Some("text/event-plain")
            || self.event_name().is_some()
    }

    /// Check if command was successful (starts with +OK)
    pub fn is_ok(&self) -> bool {
        self.reply_text()
            .map(|t| t.starts_with("+OK"))
            .unwrap_or(false)
    }

    /// Check if command failed (starts with -ERR)
    pub fn is_error(&self) -> bool {
        self.reply_text()
            .map(|t| t.starts_with("-ERR"))
            .unwrap_or(false)
    }

    /// Get error message from -ERR response
    pub fn error_message(&self) -> Option<String> {
        if self.is_error() {
            self.reply_text().map(|t| {
                t.strip_prefix("-ERR ")
                    .unwrap_or(t)
                    .to_string()
            })
        } else {
            None
        }
    }

    /// Get custom variable value
    pub fn get_variable(&self, name: &str) -> Option<&str> {
        let key = format!("variable_{}", name);
        self.get_header(&key)
    }
}

impl fmt::Display for EslEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EslEvent {{")?;

        if let Some(event_name) = self.event_name() {
            write!(f, " Event-Name: {}", event_name)?;
        }

        if let Some(uuid) = self.unique_id() {
            write!(f, ", UUID: {}", uuid)?;
        }

        if let Some(caller) = self.caller_number() {
            write!(f, ", Caller: {}", caller)?;
        }

        if let Some(dest) = self.destination_number() {
            write!(f, ", Destination: {}", dest)?;
        }

        write!(f, ", Headers: {} }}", self.headers.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_event() {
        let event = EslEvent::parse("");
        assert!(event.headers.is_empty());
        assert!(event.body.is_none());
    }

    #[test]
    fn test_parse_simple_event() {
        let raw = "Event-Name: CHANNEL_CREATE\nUnique-ID: 12345\n";
        let event = EslEvent::parse(raw);

        assert_eq!(event.event_name(), Some("CHANNEL_CREATE"));
        assert_eq!(event.unique_id(), Some("12345"));
    }

    #[test]
    fn test_parse_with_body() {
        let raw = "Content-Type: command/reply\nReply-Text: +OK\n\nBody content";
        let event = EslEvent::parse(raw);

        assert_eq!(event.content_type(), Some("command/reply"));
        assert_eq!(event.reply_text(), Some("+OK"));
        assert_eq!(event.body(), Some("Body content"));
    }

    #[test]
    fn test_url_decoding() {
        let raw = "Caller-Destination-Number: 1234%20test\n";
        let event = EslEvent::parse(raw);

        assert_eq!(event.destination_number(), Some("1234 test"));
    }

    #[test]
    fn test_is_ok() {
        let mut event = EslEvent::new();
        event.set_header("Reply-Text".to_string(), "+OK accepted".to_string());
        assert!(event.is_ok());
        assert!(!event.is_error());
    }

    #[test]
    fn test_is_error() {
        let mut event = EslEvent::new();
        event.set_header("Reply-Text".to_string(), "-ERR invalid command".to_string());
        assert!(event.is_error());
        assert!(!event.is_ok());
        assert_eq!(event.error_message(), Some("invalid command".to_string()));
    }

    #[test]
    fn test_billsec_parsing() {
        let mut event = EslEvent::new();
        event.set_header("variable_billsec".to_string(), "123".to_string());
        assert_eq!(event.billsec(), Some(123));
    }

    #[test]
    fn test_caller_number_fallback() {
        let mut event = EslEvent::new();
        event.set_header("variable_sip_from_user".to_string(), "5551234".to_string());
        assert_eq!(event.caller_number(), Some("5551234"));
    }

    #[test]
    fn test_get_variable() {
        let mut event = EslEvent::new();
        event.set_header("variable_custom_data".to_string(), "test_value".to_string());
        assert_eq!(event.get_variable("custom_data"), Some("test_value"));
    }
}
