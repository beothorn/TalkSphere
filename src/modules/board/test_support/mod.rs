//! Deterministic test-only fixtures and durable-effect observations; never production adapters.
use crate::modules::board::Error;
use std::collections::VecDeque;

/// Failure cut point for a test-owned atomic durable write.
#[derive(Clone, Copy)]
pub(super) enum Cut {
    BeforeCommit,
    AfterCommit,
    Acknowledged,
}
/// Retain durable bytes across simulated restart independently of volatile observations.
#[derive(Default)]
pub(super) struct Store {
    pub bytes: Option<Vec<u8>>,
    pub attempts: usize,
    pub acknowledgements: usize,
    pub cuts: VecDeque<Cut>,
}
impl Store {
    // Record an attempted write and distinguish no commit from a lost postcommit acknowledgement.
    pub fn commit(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.attempts += 1;
        match self.cuts.pop_front().ok_or(Error::Storage)? {
            Cut::BeforeCommit => Err(Error::Storage),
            Cut::AfterCommit => {
                self.bytes = Some(bytes.to_vec());
                Err(Error::Unknown)
            }
            Cut::Acknowledged => {
                self.bytes = Some(bytes.to_vec());
                self.acknowledgements += 1;
                Ok(())
            }
        }
    }
    // Restart drops scripted responses and volatile counters, preserving only committed bytes.
    pub fn restart(&self) -> Self {
        Self {
            bytes: self.bytes.clone(),
            ..Self::default()
        }
    }
}
/// Controllable local time; no wall clock or sleeping is needed for cache tests.
#[derive(Default)]
pub(super) struct Clock(pub u64);
impl Clock {
    // Advance monotonic fixture seconds, rejecting overflow without changing time.
    pub fn advance(&mut self, seconds: u64) -> Result<(), Error> {
        self.0 = self.0.checked_add(seconds).ok_or(Error::Limit)?;
        Ok(())
    }
}
/// Independent literal price expectations, not calculated through the Board decoder.
pub(super) const PRICES: [u64; 3] = [7, 9_007_199_254_740_993, 18_446_744_073_709_551_615];

// Construct complete schema documents with matching identity fields, without a production encoder.
pub(super) fn definition() -> &'static str {
    r#"{"id":"fixture","service":"org.example.echo","serviceVersion":1,"description":"Return the supplied label.","input":{"type":"string"},"output":{"type":"string"},"check":{"description":"Compare the original label with the returned label.","input":{"type":"string"},"output":{"type":"bool"}}}"#
}
// Extract the public pinned identity from upstream Transport fixtures; this grants no authentication.
pub(super) fn issuer() -> String {
    use crate::modules::identity::test_support::{TRANSPORT, vector};
    let bytes: [u8; 32] = vector(TRANSPORT, "initiator CR2SE ID").try_into().unwrap();
    crate::modules::identity::text::format(crate::modules::identity::api::IdentityId(bytes))
}
// Populate either offering direction with literal custom terms and a real fixture-derived textual ID.
pub(super) fn board(wanted: bool, price: u64) -> String {
    let offering = format!(
        r#"{{"id":"fixture","service":"org.example.echo","serviceVersion":1,"description":"Return the supplied label.","creditIssuer":"{}","price":{price}}}"#,
        issuer()
    );
    let (provided, wanted) = if wanted {
        (String::new(), offering)
    } else {
        (offering, String::new())
    };
    format!(r#"{{"version":1,"providedServices":[{provided}],"wantedServices":[{wanted}]}}"#)
}

/// Explicit fixture capabilities; unknown semantics never become supported by default.
#[derive(Default)]
pub(super) struct Capabilities {
    pub services: std::collections::BTreeSet<(String, u32)>,
    pub preconditions: std::collections::BTreeSet<String>,
    pub price: Option<Result<u64, Error>>,
}
impl Capabilities {
    // Return only the declared exact version and required semantics, without external effects.
    pub fn usable(&self, service: &str, version: u32, required: &[&str]) -> bool {
        self.services.contains(&(service.to_owned(), version))
            && required
                .iter()
                .all(|name| self.preconditions.contains(*name))
    }
    // Supply a literal pricing outcome; absent model support is explicitly unsupported.
    pub fn price(&self) -> Result<u64, Error> {
        self.price.unwrap_or(Err(Error::Unsupported))
    }
}

// A test-owned handler checks its contract against literal schemas, never trusting matching names alone.
pub(super) fn contract(d: &super::definition::Definition) -> Result<(), Error> {
    let expected: serde_json::Value = serde_json::from_str(definition()).unwrap();
    if ["input", "output", "check"]
        .iter()
        .all(|key| d.document.value().get(*key) == expected.get(*key))
    {
        Ok(())
    } else {
        Err(Error::Invalid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The helper must distinguish old state, committed unknown state and acknowledged state on restart.
    #[test]
    fn commit_cuts_preserve_only_durable_effects() {
        let mut store = Store {
            cuts: VecDeque::from([Cut::Acknowledged, Cut::BeforeCommit, Cut::AfterCommit]),
            ..Store::default()
        };
        assert_eq!(store.commit(b"old"), Ok(()));
        assert_eq!(store.commit(b"rejected"), Err(Error::Storage));
        assert_eq!(store.restart().bytes.as_deref(), Some(&b"old"[..]));
        assert_eq!(store.commit(b"new"), Err(Error::Unknown));
        assert_eq!(store.restart().bytes.as_deref(), Some(&b"new"[..]));
        assert_eq!((store.attempts, store.acknowledgements), (3, 1));
        assert_eq!(store.commit(b"unscripted"), Err(Error::Storage));
    }
    // Exact supported versions/preconditions and scripted arithmetic failures do not imply real providers.
    #[test]
    fn capability_fixture_requires_explicit_support() {
        let mut caps = Capabilities::default();
        assert!(!caps.usable("org.example.echo", 1, &[]));
        assert_eq!(caps.price(), Err(Error::Unsupported));
        caps.services.insert(("org.example.echo".into(), 1));
        assert!(caps.usable("org.example.echo", 1, &[]));
        assert!(!caps.usable("org.example.echo", 2, &[]));
        assert!(!caps.usable("org.example.echo", 1, &["cr2se.identity"]));
        caps.preconditions.insert("cr2se.identity".into());
        assert!(caps.usable("org.example.echo", 1, &["cr2se.identity"]));
        caps.price = Some(Err(Error::Limit));
        assert_eq!(caps.price(), Err(Error::Limit));
        caps.price = Some(Ok(7));
        assert_eq!(caps.price(), Ok(7));
    }
    // Clock overflow changes no time; deterministic literal documents contain independent exact prices.
    #[test]
    fn literal_fixtures_and_clock_are_stable() {
        let mut clock = Clock::default();
        clock.advance(1000).unwrap();
        assert_eq!(clock.advance(u64::MAX), Err(Error::Limit));
        assert_eq!(clock.0, 1000);
        assert!(definition().contains("\"check\""));
        assert!(crate::modules::identity::text::parse(&issuer()).is_ok());
        for price in PRICES {
            assert!(board(false, price).contains(&format!("\"price\":{price}")));
            assert!(board(true, price).contains("\"providedServices\":[]"));
        }
    }
}
