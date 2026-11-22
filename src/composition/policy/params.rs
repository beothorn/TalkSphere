//! Validated policy parameters read from `policy.json`.
//!
//! Callers: node composition and `node-ctl policy show`. The document is trusted operator
//! input; validation happens before any effect, every parameter has a finite range, and
//! omitted members keep the conservative defaults that reproduce the minimum node. Error
//! messages name members, never values.
use super::contracts::Wanted;
use serde_json::{Map, Value, json};

/// Largest accepted document.
pub const MAXIMUM_DOCUMENT_BYTES: usize = 16384;
/// Largest replica target.
pub const MAXIMUM_TARGET: u64 = 64;
/// Marker for "no rate limit" used when the document sets none.
pub const UNLIMITED: u32 = u32::MAX;

/// Failure to accept a policy document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamsError {
    /// Not a bounded JSON object.
    Malformed,
    /// A member is unknown or of the wrong type.
    Member(String),
    /// A value is outside its range or inconsistent with another.
    Value(String),
}
impl std::fmt::Display for ParamsError {
    // Name the member only.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed => write!(f, "policy document is not a valid JSON object"),
            Self::Member(name) => write!(f, "policy member {name:?} is not valid"),
            Self::Value(name) => write!(f, "policy value {name:?} is out of range"),
        }
    }
}

/// Connection admission limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmissionParams {
    /// Live connections the node keeps.
    pub connections: usize,
    /// Live connections one identity may hold.
    pub per_peer_connections: usize,
}
/// Per-identity quotas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuotaParams {
    /// Simultaneous pending operations.
    pub pending: usize,
    /// Requests per rolling minute.
    pub requests_per_minute: u32,
    /// Failed attempts per rolling minute.
    pub failures_per_minute: u32,
}
/// Automatic trust steps in millionths; all zero leaves trust to the operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustParams {
    /// Raise after a passed check.
    pub check_passed: u32,
    /// Lower after a failed check.
    pub check_failed: u32,
    /// Raise after a verified contribution.
    pub contribution: u32,
    /// Lower after an attributable failure.
    pub failure: u32,
    /// Automatic raises never take trust above this value.
    pub ceiling: u32,
}
/// Credit issuance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreditParams {
    /// Credits in a new identity's first grant; zero disables bootstrap.
    pub bootstrap_credits: u64,
    /// Trust given with the first grant.
    pub bootstrap_trust: u32,
    /// Total credits bootstrap may ever issue.
    pub bootstrap_budget: u64,
    /// Wanted Storage terms to publish.
    pub wanted: Option<Wanted>,
    /// Share of cumulative payments offered back on early removal, in percent.
    pub release_percent: u32,
}
/// Replica placement and cadence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RedundancyParams {
    /// Independent confirmed agreements wanted per item.
    pub target: usize,
    /// Seconds between checks of one replica.
    pub check_seconds: u64,
    /// Offset per replica index, in percent of the check interval, up to 90.
    pub stagger_percent: u32,
    /// A replica is renewed once it has this many seconds of retention left.
    pub renew_before_seconds: u64,
    /// Most credits one check or renewal may cost, in the holder's credits.
    pub max_spend: u64,
    /// Seconds to wait before trying a check or renewal again after it could not be completed.
    pub retry_seconds: u64,
}
/// Connection upkeep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionParams {
    /// Seconds between idle pings.
    pub ping_seconds: u64,
    /// Seconds to wait for a pong.
    pub ping_timeout_seconds: u64,
    /// Largest reconnect delay in seconds.
    pub reconnect_max_seconds: u64,
}

/// Every policy parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Params {
    /// Admission section.
    pub admission: AdmissionParams,
    /// Quota section.
    pub quota: QuotaParams,
    /// Trust section.
    pub trust: TrustParams,
    /// Credit section.
    pub credit: CreditParams,
    /// Redundancy section.
    pub redundancy: RedundancyParams,
    /// Connection section.
    pub connection: ConnectionParams,
}

impl Default for Params {
    // The values that reproduce the minimum node.
    fn default() -> Self {
        tracing::trace!("Building the default policy parameters");
        Self {
            admission: AdmissionParams {
                connections: 64,
                per_peer_connections: 64,
            },
            quota: QuotaParams {
                pending: 8,
                requests_per_minute: UNLIMITED,
                failures_per_minute: UNLIMITED,
            },
            trust: TrustParams {
                check_passed: 0,
                check_failed: 0,
                contribution: 0,
                failure: 0,
                ceiling: 0,
            },
            credit: CreditParams {
                bootstrap_credits: 0,
                bootstrap_trust: 0,
                bootstrap_budget: 0,
                wanted: None,
                release_percent: 0,
            },
            redundancy: RedundancyParams {
                target: 2,
                check_seconds: 3600,
                stagger_percent: 10,
                renew_before_seconds: 600,
                max_spend: 100,
                retry_seconds: 60,
            },
            connection: ConnectionParams {
                ping_seconds: 30,
                ping_timeout_seconds: 10,
                reconnect_max_seconds: 60,
            },
        }
    }
}

impl Params {
    /// Parse and validate a policy document; omitted members keep their defaults.
    /// # Errors
    /// Malformed JSON, unknown members, wrong types and out-of-range or inconsistent values.
    pub fn parse(bytes: &[u8]) -> Result<Self, ParamsError> {
        tracing::trace!("Parsing the policy document");
        if bytes.len() > MAXIMUM_DOCUMENT_BYTES {
            return Err(ParamsError::Malformed);
        }
        let Value::Object(root) =
            serde_json::from_slice(bytes).map_err(|_| ParamsError::Malformed)?
        else {
            return Err(ParamsError::Malformed);
        };
        names(
            &root,
            &[
                "version",
                "admission",
                "quota",
                "trust",
                "credit",
                "redundancy",
                "connection",
            ],
            "",
        )?;
        if root.get("version").and_then(Value::as_u64) != Some(1) {
            return Err(ParamsError::Member("version".into()));
        }
        let mut p = Self::default();
        if let Some(s) = section(&root, "admission")? {
            names(s, &["connections", "perPeerConnections"], "admission")?;
            set(
                &mut p.admission.connections,
                s,
                "admission",
                "connections",
                1,
                4096,
            )?;
            set(
                &mut p.admission.per_peer_connections,
                s,
                "admission",
                "perPeerConnections",
                1,
                4096,
            )?;
        }
        if let Some(s) = section(&root, "quota")? {
            names(
                s,
                &["pending", "requestsPerMinute", "failuresPerMinute"],
                "quota",
            )?;
            set(&mut p.quota.pending, s, "quota", "pending", 1, 4096)?;
            set_rate(&mut p.quota.requests_per_minute, s, "requestsPerMinute")?;
            set_rate(&mut p.quota.failures_per_minute, s, "failuresPerMinute")?;
        }
        if let Some(s) = section(&root, "trust")? {
            names(
                s,
                &[
                    "checkPassed",
                    "checkFailed",
                    "contribution",
                    "failure",
                    "ceiling",
                ],
                "trust",
            )?;
            set(
                &mut p.trust.check_passed,
                s,
                "trust",
                "checkPassed",
                0,
                1_000_000,
            )?;
            set(
                &mut p.trust.check_failed,
                s,
                "trust",
                "checkFailed",
                0,
                1_000_000,
            )?;
            set(
                &mut p.trust.contribution,
                s,
                "trust",
                "contribution",
                0,
                1_000_000,
            )?;
            set(&mut p.trust.failure, s, "trust", "failure", 0, 1_000_000)?;
            set(&mut p.trust.ceiling, s, "trust", "ceiling", 0, 1_000_000)?;
        }
        if let Some(s) = section(&root, "credit")? {
            parse_credit(s, &mut p.credit)?;
        }
        if let Some(s) = section(&root, "redundancy")? {
            names(
                s,
                &[
                    "target",
                    "checkSeconds",
                    "staggerPercent",
                    "renewBeforeSeconds",
                    "maxSpend",
                    "retrySeconds",
                ],
                "redundancy",
            )?;
            set(
                &mut p.redundancy.renew_before_seconds,
                s,
                "redundancy",
                "renewBeforeSeconds",
                1,
                31_536_000,
            )?;
            set(
                &mut p.redundancy.max_spend,
                s,
                "redundancy",
                "maxSpend",
                1,
                u64::MAX,
            )?;
            set(
                &mut p.redundancy.retry_seconds,
                s,
                "redundancy",
                "retrySeconds",
                1,
                86_400,
            )?;
            set(
                &mut p.redundancy.target,
                s,
                "redundancy",
                "target",
                1,
                MAXIMUM_TARGET,
            )?;
            set(
                &mut p.redundancy.check_seconds,
                s,
                "redundancy",
                "checkSeconds",
                1,
                31_536_000,
            )?;
            set(
                &mut p.redundancy.stagger_percent,
                s,
                "redundancy",
                "staggerPercent",
                0,
                90,
            )?;
        }
        if let Some(s) = section(&root, "connection")? {
            names(
                s,
                &["pingSeconds", "pingTimeoutSeconds", "reconnectMaxSeconds"],
                "connection",
            )?;
            set(
                &mut p.connection.ping_seconds,
                s,
                "connection",
                "pingSeconds",
                1,
                3600,
            )?;
            set(
                &mut p.connection.ping_timeout_seconds,
                s,
                "connection",
                "pingTimeoutSeconds",
                1,
                3600,
            )?;
            set(
                &mut p.connection.reconnect_max_seconds,
                s,
                "connection",
                "reconnectMaxSeconds",
                1,
                3600,
            )?;
        }
        p.validate()?;
        Ok(p)
    }

    /// Read the policy document at `path`; an absent file selects the defaults.
    /// # Errors
    /// An unreadable or oversized file, or any parse or validation failure.
    pub fn load(path: &std::path::Path) -> Result<Self, ParamsError> {
        tracing::trace!("Loading the policy document");
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(_) => return Err(ParamsError::Malformed),
        };
        Self::parse(&bytes)
    }

    /// Check cross-field consistency.
    /// # Errors
    /// Inconsistent values are rejected, naming the first member involved.
    pub fn validate(&self) -> Result<(), ParamsError> {
        tracing::trace!("Validating the policy parameters");
        let bad = |name: &str| Err(ParamsError::Value(name.into()));
        if self.admission.per_peer_connections > self.admission.connections {
            return bad("admission.perPeerConnections");
        }
        if self.connection.ping_timeout_seconds > self.connection.ping_seconds {
            return bad("connection.pingTimeoutSeconds");
        }
        let t = &self.trust;
        if t.ceiling == 0 && (t.check_passed > 0 || t.contribution > 0) {
            return bad("trust.ceiling");
        }
        let c = &self.credit;
        if c.bootstrap_credits > c.bootstrap_budget {
            return bad("credit.bootstrapCredits");
        }
        if c.bootstrap_credits == 0 && (c.bootstrap_budget > 0 || c.bootstrap_trust > 0) {
            return bad("credit.bootstrapCredits");
        }
        if c.bootstrap_credits > 0 && c.bootstrap_trust == 0 {
            return bad("credit.bootstrapTrust");
        }
        if let Some(w) = &c.wanted
            && (w.credits_per_object == 0
                || w.maximum_object_bytes == 0
                || w.retention_seconds == 0)
        {
            return bad("credit.wanted");
        }
        Ok(())
    }

    /// Serialize to a canonical document that parses back to the same parameters.
    pub fn to_json(&self) -> Value {
        tracing::trace!("Serializing the policy parameters");
        let wanted = self.credit.wanted.map_or(Value::Null, |w| {
            json!({
                "creditsPerObject": w.credits_per_object,
                "maximumObjectBytes": w.maximum_object_bytes,
                "retentionSeconds": w.retention_seconds,
            })
        });
        json!({
            "version": 1,
            "admission": {
                "connections": self.admission.connections,
                "perPeerConnections": self.admission.per_peer_connections,
            },
            "quota": {
                "pending": self.quota.pending,
                "requestsPerMinute": unlimited(self.quota.requests_per_minute),
                "failuresPerMinute": unlimited(self.quota.failures_per_minute),
            },
            "trust": {
                "checkPassed": self.trust.check_passed, "checkFailed": self.trust.check_failed,
                "contribution": self.trust.contribution, "failure": self.trust.failure,
                "ceiling": self.trust.ceiling,
            },
            "credit": {
                "bootstrapCredits": self.credit.bootstrap_credits,
                "bootstrapTrust": self.credit.bootstrap_trust,
                "bootstrapBudget": self.credit.bootstrap_budget,
                "wanted": wanted,
                "releasePercent": self.credit.release_percent,
            },
            "redundancy": {
                "target": self.redundancy.target,
                "checkSeconds": self.redundancy.check_seconds,
                "staggerPercent": self.redundancy.stagger_percent,
                "renewBeforeSeconds": self.redundancy.renew_before_seconds,
                "maxSpend": self.redundancy.max_spend,
                "retrySeconds": self.redundancy.retry_seconds,
            },
            "connection": {
                "pingSeconds": self.connection.ping_seconds,
                "pingTimeoutSeconds": self.connection.ping_timeout_seconds,
                "reconnectMaxSeconds": self.connection.reconnect_max_seconds,
            },
        })
    }
}

// An unset rate prints as null, because no limit applies.
fn unlimited(value: u32) -> Value {
    if value == UNLIMITED {
        Value::Null
    } else {
        value.into()
    }
}

// Read the credit section, including the optional wanted terms.
fn parse_credit(s: &Map<String, Value>, credit: &mut CreditParams) -> Result<(), ParamsError> {
    tracing::trace!("Parsing the credit policy section");
    names(
        s,
        &[
            "bootstrapCredits",
            "bootstrapTrust",
            "bootstrapBudget",
            "wanted",
            "releasePercent",
        ],
        "credit",
    )?;
    set(
        &mut credit.bootstrap_credits,
        s,
        "credit",
        "bootstrapCredits",
        0,
        u64::MAX,
    )?;
    set(
        &mut credit.bootstrap_trust,
        s,
        "credit",
        "bootstrapTrust",
        0,
        1_000_000,
    )?;
    set(
        &mut credit.bootstrap_budget,
        s,
        "credit",
        "bootstrapBudget",
        0,
        u64::MAX,
    )?;
    set(
        &mut credit.release_percent,
        s,
        "credit",
        "releasePercent",
        0,
        100,
    )?;
    match s.get("wanted") {
        None => {}
        Some(Value::Null) => credit.wanted = None,
        Some(Value::Object(w)) => {
            names(
                w,
                &["creditsPerObject", "maximumObjectBytes", "retentionSeconds"],
                "credit.wanted",
            )?;
            let mut terms = Wanted {
                credits_per_object: 0,
                maximum_object_bytes: 0,
                retention_seconds: 0,
            };
            for (field, slot) in [
                ("creditsPerObject", &mut terms.credits_per_object),
                ("maximumObjectBytes", &mut terms.maximum_object_bytes),
                ("retentionSeconds", &mut terms.retention_seconds),
            ] {
                let value = w
                    .get(field)
                    .and_then(Value::as_u64)
                    .ok_or_else(|| ParamsError::Value(format!("credit.wanted.{field}")))?;
                *slot = value;
            }
            credit.wanted = Some(terms);
        }
        Some(_) => return Err(ParamsError::Member("credit.wanted".into())),
    }
    Ok(())
}

// The named optional section as an object.
fn section<'a>(
    root: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a Map<String, Value>>, ParamsError> {
    match root.get(name) {
        None => Ok(None),
        Some(Value::Object(members)) => Ok(Some(members)),
        Some(_) => Err(ParamsError::Member(name.into())),
    }
}

// Refuse every member not in `allowed`.
fn names(members: &Map<String, Value>, allowed: &[&str], prefix: &str) -> Result<(), ParamsError> {
    for name in members.keys() {
        if !allowed.contains(&name.as_str()) {
            let full = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}.{name}")
            };
            return Err(ParamsError::Member(full));
        }
    }
    Ok(())
}

// A rate is a positive integer, or null for no limit.
fn set_rate(slot: &mut u32, members: &Map<String, Value>, name: &str) -> Result<(), ParamsError> {
    if members.get(name).is_some_and(Value::is_null) {
        *slot = UNLIMITED;
        return Ok(());
    }
    set(slot, members, "quota", name, 1, 1_000_000)
}

// Replace `slot` with the member's exact integer when present and within `min..=max`.
fn set<T: TryFrom<u64>>(
    slot: &mut T,
    members: &Map<String, Value>,
    section: &str,
    name: &str,
    min: u64,
    max: u64,
) -> Result<(), ParamsError> {
    let Some(value) = members.get(name) else {
        return Ok(());
    };
    let bad = || ParamsError::Value(format!("{section}.{name}"));
    let n = value
        .as_u64()
        .filter(|n| (min..=max).contains(n))
        .ok_or_else(bad)?;
    *slot = T::try_from(n).map_err(|_| bad())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Parse a document built from a JSON literal.
    fn parse(text: &str) -> Result<Params, ParamsError> {
        Params::parse(text.as_bytes())
    }

    // A document with only the version keeps every default.
    #[test]
    fn a_minimal_document_keeps_the_defaults() {
        assert_eq!(parse(r#"{"version":1}"#).unwrap(), Params::default());
    }

    // The canonical document parses back to the same parameters, including a wanted offering.
    #[test]
    fn serialization_round_trips() {
        let mut p = Params::default();
        p.quota.requests_per_minute = 600;
        p.credit.bootstrap_credits = 10;
        p.credit.bootstrap_budget = 1000;
        p.credit.bootstrap_trust = 1;
        p.credit.wanted = Some(Wanted {
            credits_per_object: 3,
            maximum_object_bytes: 4096,
            retention_seconds: 3600,
        });
        let text = p.to_json().to_string();
        assert_eq!(parse(&text).unwrap(), p);
        assert_eq!(
            parse(&Params::default().to_json().to_string()).unwrap(),
            Params::default()
        );
    }

    // Missing or wrong versions, unknown members and non-objects are refused by member name.
    #[test]
    fn structure_errors_name_the_member() {
        assert_eq!(parse("[]"), Err(ParamsError::Malformed));
        assert_eq!(parse("{}"), Err(ParamsError::Member("version".into())));
        assert_eq!(
            parse(r#"{"version":2}"#),
            Err(ParamsError::Member("version".into()))
        );
        assert_eq!(
            parse(r#"{"version":1,"extra":1}"#),
            Err(ParamsError::Member("extra".into()))
        );
        assert_eq!(
            parse(r#"{"version":1,"quota":{"nope":1}}"#),
            Err(ParamsError::Member("quota.nope".into()))
        );
        assert_eq!(
            parse(r#"{"version":1,"quota":3}"#),
            Err(ParamsError::Member("quota".into()))
        );
    }

    // Values outside their ranges are refused: zero, oversized, fractional, negative and typed wrongly.
    #[test]
    fn out_of_range_values_are_refused() {
        for (doc, member) in [
            (r#"{"version":1,"quota":{"pending":0}}"#, "quota.pending"),
            (r#"{"version":1,"quota":{"pending":5000}}"#, "quota.pending"),
            (
                r#"{"version":1,"quota":{"requestsPerMinute":1.5}}"#,
                "quota.requestsPerMinute",
            ),
            (
                r#"{"version":1,"redundancy":{"target":65}}"#,
                "redundancy.target",
            ),
            (
                r#"{"version":1,"redundancy":{"checkSeconds":0}}"#,
                "redundancy.checkSeconds",
            ),
            (
                r#"{"version":1,"redundancy":{"renewBeforeSeconds":0}}"#,
                "redundancy.renewBeforeSeconds",
            ),
            (
                r#"{"version":1,"redundancy":{"maxSpend":0}}"#,
                "redundancy.maxSpend",
            ),
            (
                r#"{"version":1,"redundancy":{"retrySeconds":86401}}"#,
                "redundancy.retrySeconds",
            ),
            (
                r#"{"version":1,"redundancy":{"staggerPercent":91}}"#,
                "redundancy.staggerPercent",
            ),
            (
                r#"{"version":1,"trust":{"ceiling":1000001}}"#,
                "trust.ceiling",
            ),
            (
                r#"{"version":1,"credit":{"releasePercent":101}}"#,
                "credit.releasePercent",
            ),
            (
                r#"{"version":1,"connection":{"pingSeconds":-1}}"#,
                "connection.pingSeconds",
            ),
        ] {
            assert_eq!(parse(doc), Err(ParamsError::Value(member.into())), "{doc}");
        }
    }

    // Inconsistent combinations are refused even when each value is individually valid.
    #[test]
    fn inconsistent_values_are_refused() {
        for (doc, member) in [
            (
                r#"{"version":1,"admission":{"connections":4,"perPeerConnections":5}}"#,
                "admission.perPeerConnections",
            ),
            (
                r#"{"version":1,"connection":{"pingSeconds":5,"pingTimeoutSeconds":6}}"#,
                "connection.pingTimeoutSeconds",
            ),
            (
                r#"{"version":1,"trust":{"checkPassed":10}}"#,
                "trust.ceiling",
            ),
            (
                r#"{"version":1,"credit":{"bootstrapCredits":10,"bootstrapBudget":5,"bootstrapTrust":1}}"#,
                "credit.bootstrapCredits",
            ),
            (
                r#"{"version":1,"credit":{"bootstrapCredits":10,"bootstrapBudget":10}}"#,
                "credit.bootstrapTrust",
            ),
            (
                r#"{"version":1,"credit":{"bootstrapBudget":10}}"#,
                "credit.bootstrapCredits",
            ),
            (
                r#"{"version":1,"credit":{"wanted":{"creditsPerObject":0,"maximumObjectBytes":1,"retentionSeconds":1}}}"#,
                "credit.wanted",
            ),
            (
                r#"{"version":1,"credit":{"wanted":{"creditsPerObject":1,"maximumObjectBytes":1}}}"#,
                "credit.wanted.retentionSeconds",
            ),
        ] {
            assert_eq!(parse(doc), Err(ParamsError::Value(member.into())), "{doc}");
        }
    }

    // An absent policy file selects the defaults; a present one is parsed; an unreadable one is refused.
    #[test]
    fn loading_handles_absent_present_and_unreadable_files() {
        let dir = std::env::temp_dir().join(format!("ts-policy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(
            Params::load(&dir.join("missing.json")).unwrap(),
            Params::default()
        );
        let file = dir.join("policy.json");
        std::fs::write(&file, r#"{"version":1,"redundancy":{"target":7}}"#).unwrap();
        assert_eq!(Params::load(&file).unwrap().redundancy.target, 7);
        std::fs::write(&file, "nope").unwrap();
        assert_eq!(Params::load(&file), Err(ParamsError::Malformed));
        // A directory where the file belongs is unreadable, not silently defaulted.
        assert_eq!(Params::load(&dir), Err(ParamsError::Malformed));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // A document larger than the bound is refused before parsing.
    #[test]
    fn oversized_documents_are_refused() {
        let mut text = String::from(r#"{"version":1,"x":""#);
        text.push_str(&"a".repeat(MAXIMUM_DOCUMENT_BYTES));
        text.push_str(r#""}"#);
        assert_eq!(parse(&text), Err(ParamsError::Malformed));
    }
}
