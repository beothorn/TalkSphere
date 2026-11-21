//! Effect-free validated node configuration read from `node.json`.
//!
//! Callers: `node-init`, `node-show` and the node runtime. The document is trusted
//! operator input; validation happens before any filesystem or network effect, and
//! the Storage offering terms are checked again by the Board and Storage owners.
use serde_json::{Map, Value};

/// Largest object the finite profile can carry; bounded by the logical value budget.
pub const MAXIMUM_OBJECT_BYTES: u64 = 16384;
/// Largest accepted configuration document.
pub const MAXIMUM_DOCUMENT_BYTES: usize = 16384;

/// Failure to accept a configuration document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// The document is not valid bounded JSON of the expected shape.
    Malformed,
    /// A member is unknown, duplicated or of the wrong type.
    Member(String),
    /// A value is outside its finite range or inconsistent with another value.
    Value(String),
}
impl std::fmt::Display for ConfigError {
    // Name the offending member only; values never appear in the message.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed => write!(f, "node configuration is not a valid JSON object"),
            Self::Member(name) => write!(f, "node configuration member {name:?} is not valid"),
            Self::Value(name) => write!(f, "node configuration value {name:?} is out of range"),
        }
    }
}

/// Finite provider capacity, independent of the maxima advertised in the offering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capacity {
    /// Aggregate live and staged object bytes.
    pub bytes: u64,
    /// Retained lease slots.
    pub leases: usize,
    /// Simultaneous pending operations.
    pub operations: usize,
    /// Aggregate retained result bytes.
    pub result_bytes: u64,
    /// Simultaneous operations for one requester.
    pub per_owner: usize,
}

/// Credit policy the operator accepts; peers never select it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Credits {
    /// Whether third-party issuer settlement is supported.
    pub third_party: bool,
    /// Exact issuer transfer fee approved for third-party settlement.
    pub fee: u64,
    /// Total credits this node may ever issue to peers.
    pub issuance: u64,
    /// Internal Ledger budget for claims and transfers.
    pub internal: u64,
}

/// Local control channel bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Control {
    /// Simultaneous control connections.
    pub connections: usize,
    /// Longest accepted request line in bytes.
    pub line_bytes: usize,
    /// Longest time one request may run, in seconds.
    pub request_seconds: u64,
}

/// Validated node policy.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeConfig {
    /// Storage offering terms; `None` makes this node requester-only and publishes nothing.
    pub offering: Option<Terms>,
    /// Provider capacity limits.
    pub capacity: Capacity,
    /// Credit settlement policy.
    pub credits: Credits,
    /// Local control bounds.
    pub control: Control,
}

/// Storage offering terms as the standard definition fields (`pricing`, `checkPrice`, `info`).
#[derive(Debug, Clone, PartialEq)]
pub struct Terms {
    /// The `pricing` object.
    pub pricing: Value,
    /// Positive paid byte-check price.
    pub check_price: u64,
    /// The `info` object with advertised maxima.
    pub info: Value,
}

impl Default for NodeConfig {
    // Finite demonstration-sized defaults; maxima never exceed what the profile can carry.
    fn default() -> Self {
        tracing::trace!("Building the default node configuration");
        let fixture: Value =
            serde_json::from_str(include_str!("../../modules/storage/fixtures/offering.json"))
                .unwrap_or(Value::Null);
        let mut info = fixture["info"].clone();
        info["maximumBytes"] = MAXIMUM_OBJECT_BYTES.into();
        info["maximumRetrieveBytes"] = MAXIMUM_OBJECT_BYTES.into();
        info["maximumRetentionSeconds"] = 86400.into();
        info["maximumTotalRetentionSeconds"] = 604800.into();
        Self {
            offering: Some(Terms {
                pricing: fixture["pricing"].clone(),
                check_price: 3,
                info,
            }),
            capacity: Capacity {
                bytes: 1_048_576,
                leases: 64,
                operations: 32,
                result_bytes: 1_048_576,
                per_owner: 8,
            },
            credits: Credits {
                third_party: false,
                fee: 0,
                issuance: 1_000_000,
                internal: 10_000,
            },
            control: Control {
                connections: 4,
                line_bytes: 1_048_576,
                request_seconds: 60,
            },
        }
    }
}

impl NodeConfig {
    /// Parse and validate a configuration document; omitted sections take their defaults.
    /// # Errors
    /// Malformed JSON, duplicate or unknown members and out-of-range values are rejected.
    pub fn parse(bytes: &[u8]) -> Result<Self, ConfigError> {
        tracing::trace!("Parsing the node configuration document");
        if bytes.len() > MAXIMUM_DOCUMENT_BYTES {
            return Err(ConfigError::Malformed);
        }
        let value: Value = serde_json::from_slice(bytes).map_err(|_| ConfigError::Malformed)?;
        let Value::Object(members) = value else {
            return Err(ConfigError::Malformed);
        };
        let mut config = Self::default();
        let mut version = false;
        for (name, member) in &members {
            match name.as_str() {
                "version" => {
                    if member.as_u64() != Some(1) {
                        return Err(ConfigError::Value("version".into()));
                    }
                    version = true;
                }
                "offering" => config.offering = parse_offering(member)?,
                "capacity" => parse_capacity(member, &mut config.capacity)?,
                "credits" => parse_credits(member, &mut config.credits)?,
                "control" => parse_control(member, &mut config.control)?,
                _ => return Err(ConfigError::Member(name.clone())),
            }
        }
        if !version {
            return Err(ConfigError::Member("version".into()));
        }
        config.validate()?;
        Ok(config)
    }

    /// Check cross-field consistency of finite limits.
    /// # Errors
    /// Zero limits, a per-owner limit above the operation limit, or advertised maxima the
    /// finite profile cannot carry are rejected.
    pub fn validate(&self) -> Result<(), ConfigError> {
        tracing::trace!("Validating the node configuration");
        let capacity = &self.capacity;
        let bad = |name: &str| Err(ConfigError::Value(name.into()));
        if capacity.bytes == 0 || capacity.leases == 0 || capacity.operations == 0 {
            return bad("capacity");
        }
        if capacity.result_bytes == 0 || capacity.per_owner == 0 {
            return bad("capacity");
        }
        if capacity.per_owner > capacity.operations {
            return bad("capacity.perOwner");
        }
        let control = &self.control;
        if control.connections == 0 || control.connections > 64 {
            return bad("control.connections");
        }
        if control.line_bytes < 256 || control.line_bytes > 16 * 1_048_576 {
            return bad("control.lineBytes");
        }
        if control.request_seconds == 0 || control.request_seconds > 3600 {
            return bad("control.requestSeconds");
        }
        if let Some(terms) = &self.offering {
            if terms.check_price == 0 {
                return bad("offering.checkPrice");
            }
            for name in ["maximumBytes", "maximumRetrieveBytes"] {
                match terms.info[name].as_u64() {
                    Some(n) if n > 0 && n <= MAXIMUM_OBJECT_BYTES => {}
                    _ => return bad("offering.info"),
                }
            }
        }
        Ok(())
    }

    /// Serialize to a canonical operator document that parses back to the same policy.
    pub fn to_json(&self) -> Value {
        tracing::trace!("Serializing the node configuration");
        let offering = match &self.offering {
            None => Value::Null,
            Some(terms) => serde_json::json!({
                "pricing": terms.pricing, "checkPrice": terms.check_price, "info": terms.info
            }),
        };
        serde_json::json!({
            "version": 1,
            "offering": offering,
            "capacity": {
                "bytes": self.capacity.bytes, "leases": self.capacity.leases,
                "operations": self.capacity.operations, "resultBytes": self.capacity.result_bytes,
                "perOwner": self.capacity.per_owner,
            },
            "credits": {
                "thirdParty": self.credits.third_party, "fee": self.credits.fee,
                "issuance": self.credits.issuance, "internal": self.credits.internal,
            },
            "control": {
                "connections": self.control.connections, "lineBytes": self.control.line_bytes,
                "requestSeconds": self.control.request_seconds,
            },
        })
    }
}

// An explicit null selects requester-only; otherwise all three term members are required.
fn parse_offering(value: &Value) -> Result<Option<Terms>, ConfigError> {
    tracing::trace!("Parsing node offering terms");
    if value.is_null() {
        return Ok(None);
    }
    let members = object(value, "offering")?;
    check_names(members, &["pricing", "checkPrice", "info"], "offering")?;
    let get = |name: &str| {
        members
            .get(name)
            .ok_or_else(|| ConfigError::Member(format!("offering.{name}")))
    };
    let check_price = get("checkPrice")?
        .as_u64()
        .ok_or_else(|| ConfigError::Value("offering.checkPrice".into()))?;
    Ok(Some(Terms {
        pricing: get("pricing")?.clone(),
        check_price,
        info: get("info")?.clone(),
    }))
}

// Replace only the supplied capacity members; each must be a positive exact integer.
fn parse_capacity(value: &Value, capacity: &mut Capacity) -> Result<(), ConfigError> {
    tracing::trace!("Parsing node capacity");
    let members = object(value, "capacity")?;
    check_names(
        members,
        &["bytes", "leases", "operations", "resultBytes", "perOwner"],
        "capacity",
    )?;
    let number = |name: &str| -> Result<Option<u64>, ConfigError> {
        members.get(name).map_or(Ok(None), |v| {
            v.as_u64()
                .map(Some)
                .ok_or_else(|| ConfigError::Value(format!("capacity.{name}")))
        })
    };
    let count = |name: &str, old: usize| -> Result<usize, ConfigError> {
        number(name)?.map_or(Ok(old), |n| {
            usize::try_from(n).map_err(|_| ConfigError::Value(format!("capacity.{name}")))
        })
    };
    capacity.bytes = number("bytes")?.unwrap_or(capacity.bytes);
    capacity.result_bytes = number("resultBytes")?.unwrap_or(capacity.result_bytes);
    capacity.leases = count("leases", capacity.leases)?;
    capacity.operations = count("operations", capacity.operations)?;
    capacity.per_owner = count("perOwner", capacity.per_owner)?;
    Ok(())
}

// Replace only the supplied credit policy members.
fn parse_credits(value: &Value, credits: &mut Credits) -> Result<(), ConfigError> {
    tracing::trace!("Parsing node credit policy");
    let members = object(value, "credits")?;
    check_names(
        members,
        &["thirdParty", "fee", "issuance", "internal"],
        "credits",
    )?;
    if let Some(v) = members.get("thirdParty") {
        credits.third_party = v
            .as_bool()
            .ok_or_else(|| ConfigError::Value("credits.thirdParty".into()))?;
    }
    if let Some(v) = members.get("fee") {
        credits.fee = v
            .as_u64()
            .ok_or_else(|| ConfigError::Value("credits.fee".into()))?;
    }
    for (name, slot) in [
        ("issuance", &mut credits.issuance),
        ("internal", &mut credits.internal),
    ] {
        if let Some(v) = members.get(name) {
            *slot = v
                .as_u64()
                .ok_or_else(|| ConfigError::Value(format!("credits.{name}")))?;
        }
    }
    Ok(())
}

// Replace only the supplied control bounds.
fn parse_control(value: &Value, control: &mut Control) -> Result<(), ConfigError> {
    tracing::trace!("Parsing node control bounds");
    let members = object(value, "control")?;
    check_names(
        members,
        &["connections", "lineBytes", "requestSeconds"],
        "control",
    )?;
    let number = |name: &str| -> Result<Option<u64>, ConfigError> {
        members.get(name).map_or(Ok(None), |v| {
            v.as_u64()
                .map(Some)
                .ok_or_else(|| ConfigError::Value(format!("control.{name}")))
        })
    };
    let count = |name: &str, old: usize| -> Result<usize, ConfigError> {
        number(name)?.map_or(Ok(old), |n| {
            usize::try_from(n).map_err(|_| ConfigError::Value(format!("control.{name}")))
        })
    };
    control.connections = count("connections", control.connections)?;
    control.line_bytes = count("lineBytes", control.line_bytes)?;
    control.request_seconds = number("requestSeconds")?.unwrap_or(control.request_seconds);
    Ok(())
}

// Require a JSON object for a named section.
fn object<'a>(value: &'a Value, name: &str) -> Result<&'a Map<String, Value>, ConfigError> {
    tracing::trace!("Reading a node configuration section");
    value
        .as_object()
        .ok_or_else(|| ConfigError::Member(name.into()))
}

// Reject unknown members so a typo never silently selects a default.
fn check_names(
    members: &Map<String, Value>,
    allowed: &[&str],
    section: &str,
) -> Result<(), ConfigError> {
    tracing::trace!("Checking node configuration member names");
    match members.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(name) => Err(ConfigError::Member(format!("{section}.{name}"))),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The default policy is valid and survives a serialize/parse round trip.
    #[test]
    fn defaults_round_trip() {
        let config = NodeConfig::default();
        config.validate().unwrap();
        let bytes = serde_json::to_vec(&config.to_json()).unwrap();
        assert_eq!(NodeConfig::parse(&bytes).unwrap(), config);
    }
    // Partial documents replace only what they name; null selects requester-only.
    #[test]
    fn partial_documents_override_defaults() {
        let config = NodeConfig::parse(
            br#"{"version":1,"offering":null,"capacity":{"bytes":4096},"credits":{"fee":2}}"#,
        )
        .unwrap();
        assert!(config.offering.is_none());
        assert_eq!(config.capacity.bytes, 4096);
        assert_eq!(
            config.capacity.leases,
            NodeConfig::default().capacity.leases
        );
        assert_eq!(config.credits.fee, 2);
    }
    // Missing version, unknown or mistyped members and every zero or inconsistent limit are refused.
    #[test]
    fn invalid_documents_are_rejected() {
        for text in [
            "[]",
            "{}",
            r#"{"version":2}"#,
            r#"{"version":1,"extra":1}"#,
            r#"{"version":1,"capacity":{"bytes":0}}"#,
            r#"{"version":1,"capacity":{"bytes":"4"}}"#,
            r#"{"version":1,"capacity":{"operations":2,"perOwner":3}}"#,
            r#"{"version":1,"capacity":{"typo":1}}"#,
            r#"{"version":1,"control":{"connections":0}}"#,
            r#"{"version":1,"control":{"lineBytes":10}}"#,
            r#"{"version":1,"control":{"requestSeconds":0}}"#,
            r#"{"version":1,"credits":{"thirdParty":1}}"#,
            r#"{"version":1,"offering":{"pricing":{}}}"#,
            r#"not json"#,
        ] {
            assert!(NodeConfig::parse(text.as_bytes()).is_err(), "{text}");
        }
        assert!(NodeConfig::parse(&vec![b' '; MAXIMUM_DOCUMENT_BYTES + 1]).is_err());
    }
    // Advertised maxima above the finite profile and a free check are rejected.
    #[test]
    fn offering_limits_are_bounded() {
        let mut config = NodeConfig::default();
        config.offering.as_mut().unwrap().info["maximumBytes"] = (MAXIMUM_OBJECT_BYTES + 1).into();
        assert!(config.validate().is_err());
        let mut config = NodeConfig::default();
        config.offering.as_mut().unwrap().check_price = 0;
        assert!(config.validate().is_err());
    }
    // Errors name members but never echo values.
    #[test]
    fn errors_do_not_echo_values() {
        let error = NodeConfig::parse(br#"{"version":1,"capacity":{"bytes":0}}"#).unwrap_err();
        assert_eq!(
            error.to_string(),
            "node configuration value \"capacity\" is out of range"
        );
    }
}
