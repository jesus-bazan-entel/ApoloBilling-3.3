// src/esl/event.rs
use std::collections::HashMap;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct EslEvent {
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl EslEvent {
    pub fn parse(data: &str) -> Option<Self> {
        let mut headers = HashMap::new();
        let mut body = None;
        let mut in_body = false;
        let mut body_content = String::new();

        for line in data.lines() {
            if in_body {
                body_content.push_str(line);
                body_content.push('\n');
            } else if line.is_empty() {
                in_body = true;
            } else if let Some(pos) = line.find(':') {
                let key = line[..pos].trim().to_string();
                let value = line[pos + 1..].trim().to_string();
                headers.insert(key, value);
            }
        }

        if in_body && !body_content.is_empty() {
            let body_str = body_content.trim().to_string();
            
            // Check if this is a wrapper event (text/event-plain)
            if let Some(content_type) = headers.get("Content-Type") {
                if content_type == "text/event-plain" {
                    // Recursive parse of the body
                    if let Some(inner_event) = Self::parse(&body_str) {
                        // Merge inner headers override outer headers
                        for (k, v) in inner_event.headers {
                            headers.insert(k, v);
                        }
                        return Some(EslEvent { 
                            headers, 
                            body: inner_event.body 
                        });
                    }
                }
            }
            
            body = Some(body_str);
        }

        if headers.is_empty() {
            None
        } else {
            Some(EslEvent { headers, body })
        }
    }

    pub fn get_header(&self, key: &str) -> Option<&String> {
        self.headers.get(key)
    }

    pub fn event_name(&self) -> Option<&String> {
        self.headers.get("Event-Name")
    }

    pub fn is_event(&self, name: &str) -> bool {
        self.event_name().map_or(false, |n| n == name)
    }

    pub fn unique_id(&self) -> Option<&String> {
        self.headers.get("Unique-ID")
            .or_else(|| self.headers.get("Channel-Call-UUID"))
    }

    pub fn caller(&self) -> Option<&String> {
        self.headers.get("Caller-Caller-ID-Number")
            .or_else(|| self.headers.get("variable_sip_from_user"))
    }

    pub fn callee(&self) -> Option<&String> {
        self.headers.get("Caller-Destination-Number")
            .or_else(|| self.headers.get("variable_sip_to_user"))
    }

    pub fn duration(&self) -> Option<i32> {
        self.headers.get("variable_duration")
            .and_then(|s| s.parse().ok())
    }

    pub fn billsec(&self) -> Option<i32> {
        self.headers.get("variable_billsec")
            .and_then(|s| s.parse().ok())
    }

    pub fn hangup_cause(&self) -> Option<&String> {
        self.headers.get("Hangup-Cause")
    }

    /// Parse FreeSWITCH epoch timestamp (microseconds since epoch) to DateTime<Utc>
    //pub fn timestamp_to_datetime(&self, header_name: &str) -> Option<DateTime<Utc>> {
    //    self.headers.get(header_name)
    //        .and_then(|s| s.parse::<i64>().ok())
    //        .and_then(|micros| {
    //            // FreeSWITCH timestamps are in microseconds
    //            let secs = micros / 1_000_000;
    //            let nsecs = ((micros % 1_000_000) * 1000) as u32;
    //            NaiveDateTime::from_timestamp_opt(secs, nsecs)
    //                .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
    //        })
    //}
    /// Convert timestamp header to DateTime<Utc>
    /// Handles both microsecond timestamps (Event-Date-Timestamp) and second timestamps (variable_*_epoch)
    pub fn timestamp_to_datetime(&self, header_name: &str) -> Option<DateTime<Utc>> {
        let timestamp = self.headers.get(header_name)?.parse::<i64>().ok()?;
    
        // FreeSWITCH sometimes sends 0 → invalid
        if timestamp <= 0 {
            return None;
        }
    
        // Determine if timestamp is in seconds or microseconds based on header name
        if header_name.starts_with("variable_") && header_name.ends_with("_epoch") {
            // variable_*_epoch headers are in SECONDS
            DateTime::<Utc>::from_timestamp(timestamp, 0)
        } else {
            // Event-Date-Timestamp and other headers are in MICROSECONDS
            DateTime::<Utc>::from_timestamp_micros(timestamp)
        }
    }
    
    /// Get call start time (CHANNEL_CREATE epoch)
    pub fn start_time(&self) -> Option<DateTime<Utc>> {
        // Try variable_start_epoch first (most accurate)
        self.timestamp_to_datetime("variable_start_epoch")
            // Fall back to Event-Date-Timestamp for CHANNEL_CREATE
            .or_else(|| self.timestamp_to_datetime("Event-Date-Timestamp"))
    }

    /// Get call answer time
    pub fn answer_time(&self) -> Option<DateTime<Utc>> {
        self.timestamp_to_datetime("variable_answer_epoch")
    }

    /// Get call end time
    pub fn end_time(&self) -> Option<DateTime<Utc>> {
        // Try variable_end_epoch first
        self.timestamp_to_datetime("variable_end_epoch")
            // Fall back to current event timestamp
            .or_else(|| self.timestamp_to_datetime("Event-Date-Timestamp"))
    }
}
