//! Inbound Routing models
//!
//! Simplified inbound routing system with auto-priority.
//! Routes are matched by pattern (exact or wildcard).
//! Priority is determined by pattern length - longer patterns match first.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Destination type for routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DestinationType {
    /// Route to local extensions (user/{number}@host)
    Extension,
    /// Route via gateway (sofia/gateway/{name}/{number})
    Gateway,
}

impl Default for DestinationType {
    fn default() -> Self {
        Self::Extension
    }
}

impl std::fmt::Display for DestinationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Extension => write!(f, "extension"),
            Self::Gateway => write!(f, "gateway"),
        }
    }
}

impl std::str::FromStr for DestinationType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "extension" => Ok(Self::Extension),
            "gateway" => Ok(Self::Gateway),
            _ => Err(format!("Invalid destination type: {}", s)),
        }
    }
}

/// Inbound Route entity
///
/// Represents an inbound routing rule with auto-priority.
/// Pattern can be exact (e.g., "672017402000") or wildcard (e.g., "6720*").
/// Priority is automatically determined by pattern length - longer patterns win.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundRoute {
    /// Unique identifier
    pub id: Uuid,

    /// Route name (e.g., "ENTEL Principal", "PBX Fibertel")
    pub name: String,

    /// Description
    pub description: Option<String>,

    /// Pattern for matching (exact DID or prefix with *)
    /// Examples: "672017402000" (exact), "6720*" (wildcard)
    pub pattern: String,

    /// Source IP filter (optional, for carrier identification)
    pub source_ip: Option<String>,

    /// Destination type
    pub destination_type: DestinationType,

    /// Gateway ID (for gateway type)
    pub gateway_id: Option<Uuid>,

    /// Destination host
    pub destination_host: String,

    /// Digits to strip from destination number
    pub strip_digits: i32,

    /// Prefix to add after stripping
    pub prefix_to_add: String,

    /// Call timeout in seconds
    pub call_timeout: i32,

    /// IMS interworking mode (100rel/PRACK, timeout handlers)
    pub ims_mode: bool,

    /// Send early media (183 Session Progress with ringback)
    pub early_media: bool,

    /// Whether route is enabled
    pub enabled: bool,

    /// Sync status
    pub sync_status: String,

    /// Sync error message
    pub sync_error: Option<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl InboundRoute {
    /// Check if this is a wildcard pattern (ends with *)
    pub fn is_wildcard(&self) -> bool {
        self.pattern.ends_with('*')
    }

    /// Get the prefix part of the pattern (without the * if wildcard)
    pub fn prefix(&self) -> &str {
        if self.is_wildcard() {
            &self.pattern[..self.pattern.len() - 1]
        } else {
            &self.pattern
        }
    }

    /// Generate the regex pattern for FreeSWITCH dialplan
    pub fn regex_pattern(&self) -> String {
        if self.is_wildcard() {
            format!("^({}.+)$", self.prefix())
        } else {
            format!("^({})$", self.pattern)
        }
    }

    /// Check if a DID matches this route's pattern
    pub fn matches_did(&self, did: &str) -> bool {
        if self.is_wildcard() {
            did.starts_with(self.prefix())
        } else {
            did == self.pattern
        }
    }

    /// Get the FreeSWITCH extension name
    pub fn extension_name(&self) -> String {
        let safe_name = self.name.to_lowercase().replace(' ', "-");
        if self.is_wildcard() {
            format!("inbound-{}", safe_name)
        } else {
            format!("did-{}", self.pattern)
        }
    }

    /// Get priority based on pattern length (longer = higher priority = lower number)
    /// Used for sorting routes in dialplan order
    pub fn auto_priority(&self) -> i32 {
        // Exact patterns always have higher priority than wildcards of same length
        let base = self.pattern.len() as i32 * 1000;
        if self.is_wildcard() {
            -base + 500 // Wildcards get lower priority (higher number)
        } else {
            -base // Exact patterns get higher priority (lower number)
        }
    }
}

impl Default for InboundRoute {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: String::new(),
            description: None,
            pattern: String::new(),
            source_ip: None,
            destination_type: DestinationType::Extension,
            gateway_id: None,
            destination_host: "10.10.22.4".to_string(),
            strip_digits: 4,
            prefix_to_add: String::new(),
            call_timeout: 55,
            ims_mode: false,
            early_media: false,
            enabled: true,
            sync_status: "pending".to_string(),
            sync_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

/// Inbound Route with gateway name for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundRouteWithGateway {
    #[serde(flatten)]
    pub route: InboundRoute,
    pub gateway_name: Option<String>,
}

// Keep these for backward compatibility during migration
// They will be removed once migration is complete

/// DEPRECATED: Use InboundRoute instead
pub type Carrier = InboundRoute;

/// DEPRECATED: Use InboundRoute instead
pub type DidOverride = InboundRoute;

/// DEPRECATED: Use InboundRouteWithGateway instead
pub type CarrierWithGateway = InboundRouteWithGateway;

/// DEPRECATED: Use InboundRouteWithGateway instead
pub type DidOverrideWithCarrier = InboundRouteWithGateway;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wildcard_pattern() {
        let route = InboundRoute {
            pattern: "6720*".to_string(),
            ..Default::default()
        };

        assert!(route.is_wildcard());
        assert_eq!(route.prefix(), "6720");
        assert_eq!(route.regex_pattern(), "^(6720.+)$");
        assert!(route.matches_did("672017402000"));
        assert!(route.matches_did("6720999999"));
        assert!(!route.matches_did("672217402000"));
    }

    #[test]
    fn test_exact_pattern() {
        let route = InboundRoute {
            pattern: "672017402000".to_string(),
            ..Default::default()
        };

        assert!(!route.is_wildcard());
        assert_eq!(route.prefix(), "672017402000");
        assert_eq!(route.regex_pattern(), "^(672017402000)$");
        assert!(route.matches_did("672017402000"));
        assert!(!route.matches_did("672017402001"));
        assert!(!route.matches_did("6720174020001"));
    }

    #[test]
    fn test_auto_priority() {
        let exact_long = InboundRoute {
            pattern: "672017402000".to_string(),
            ..Default::default()
        };

        let wildcard_short = InboundRoute {
            pattern: "6720*".to_string(),
            ..Default::default()
        };

        let wildcard_longer = InboundRoute {
            pattern: "672017*".to_string(),
            ..Default::default()
        };

        // Longer exact pattern should have highest priority (lowest number)
        assert!(exact_long.auto_priority() < wildcard_short.auto_priority());
        // Longer wildcard should have higher priority than shorter wildcard
        assert!(wildcard_longer.auto_priority() < wildcard_short.auto_priority());
    }

    #[test]
    fn test_destination_type_display() {
        assert_eq!(DestinationType::Extension.to_string(), "extension");
        assert_eq!(DestinationType::Gateway.to_string(), "gateway");
    }

    #[test]
    fn test_destination_type_parse() {
        assert_eq!(
            "extension".parse::<DestinationType>().unwrap(),
            DestinationType::Extension
        );
        assert_eq!(
            "Gateway".parse::<DestinationType>().unwrap(),
            DestinationType::Gateway
        );
    }
}
