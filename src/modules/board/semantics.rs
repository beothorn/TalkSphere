//! Trusted semantic ports distinguish structural discovery from actual local capability.
use super::{Error, definition::Definition, offering::Offering};
use crate::modules::services::schema::Support;

/// Trusted composition supplies actual service/pricing/precondition support; no peer can register it.
pub trait Capabilities: Send + Sync {
    /// Whether the exact contract can be understood by this client, without version substitution.
    fn understands(&self, offering: &Offering) -> bool;
    /// Validate all known family terms and maximum arithmetic, including fixed-price restrictions.
    /// # Errors
    /// Returns Unsupported for an unknown pricing model and Invalid/Limit for invalid/overflowing terms.
    fn terms(&self, offering: &Offering) -> Result<(), Error>;
    /// Whether a named required precondition is understood and currently satisfied.
    /// authenticated is supplied by trusted composition, never a document claim.
    fn precondition(&self, name: &str, authenticated: bool) -> bool;
    /// Whether composition supplies a provider handler or wanted requester/input capability as appropriate.
    fn local(&self, offering: &Offering) -> bool;
}
/// Conservative default for offline inspection: documents confer no local capabilities.
pub struct Unavailable;
impl Capabilities for Unavailable {
    // No real service handler or requester codec is installed by document parsing.
    fn understands(&self, _: &Offering) -> bool {
        tracing::trace!("Refusing absent service semantics");
        false
    }
    // Without a model validator no family or pricing behavior is assumed.
    fn terms(&self, _: &Offering) -> Result<(), Error> {
        tracing::trace!("Refusing absent term validation");
        Err(Error::Unsupported)
    }
    // Transport authentication supplies only the named identity precondition, not other permissions.
    fn precondition(&self, name: &str, authenticated: bool) -> bool {
        tracing::trace!("Checking only authenticated identity preconditions");
        name == "cr2se.identity" && authenticated
    }
    // Neither provider handlers nor wanted input sources are present in offline inspection.
    fn local(&self, _: &Offering) -> bool {
        tracing::trace!("Refusing absent local publication capabilities");
        false
    }
}
/// Assess one structurally valid offering/definition pair without executing, quoting or spending.
/// # Errors
/// Unsupported semantics/preconditions return Unsupported; malformed/overflowing known terms propagate.
pub fn assess(
    caps: &dyn Capabilities,
    o: &Offering,
    d: &Definition,
    authenticated: bool,
    local: bool,
) -> Result<(), Error> {
    tracing::trace!("Assessing explicit offering semantics without invoking work");
    if d.support != Support::Known || !caps.understands(o) || (local && !caps.local(o)) {
        return Err(Error::Unsupported);
    }
    caps.terms(o)?;
    if let Some(required) = o.source.get("preconditions") {
        for name in required.as_array().ok_or(Error::Invalid)? {
            if !caps.precondition(name.as_str().ok_or(Error::Invalid)?, authenticated) {
                return Err(Error::Unsupported);
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::board::{config::Limits, offering::Board, test_support};
    struct Known;
    impl Capabilities for Known {
        // Only this fixture's exact custom version is understood.
        fn understands(&self, o: &Offering) -> bool {
            o.service == "org.example.echo" && o.version == 1
        }
        // A fixture maximum must remain positive uint64; real families supply their own arithmetic.
        fn terms(&self, o: &Offering) -> Result<(), Error> {
            if o.source.get("pricing").is_some() {
                Err(Error::Limit)
            } else {
                Ok(())
            }
        }
        // Reuse the conservative identity condition rather than assuming all names are satisfied.
        fn precondition(&self, n: &str, a: bool) -> bool {
            Unavailable.precondition(n, a)
        }
        // A fixture explicitly provides both requester/provider support; production does not.
        fn local(&self, _: &Offering) -> bool {
            true
        }
    }
    // A known fixture is usable, unknown models and preconditions block only their own offering.
    #[test]
    fn capability_checks_are_explicit() {
        let b = Board::parse(test_support::board(false, 7).as_bytes(), Limits::default()).unwrap();
        let mut o = b.offerings["fixture"].clone();
        let d = Definition::parse(test_support::definition().as_bytes(), &o, Limits::default())
            .unwrap();
        assert_eq!(assess(&Known, &o, &d, true, true), Ok(()));
        assert_eq!(
            assess(&Unavailable, &o, &d, true, false),
            Err(Error::Unsupported)
        );
        assert!(!Unavailable.local(&o));
        assert!(Unavailable.terms(&o).is_err());
        o.source["preconditions"] = serde_json::json!(["cr2se.identity"]);
        assert_eq!(
            assess(&Known, &o, &d, false, false),
            Err(Error::Unsupported)
        );
        assert_eq!(assess(&Known, &o, &d, true, false), Ok(()));
        o.source["preconditions"] = serde_json::json!(["unknown.precondition"]);
        assert_eq!(assess(&Known, &o, &d, true, false), Err(Error::Unsupported));
        o.source.as_object_mut().unwrap().remove("preconditions");
        o.source["pricing"] = serde_json::json!({"model":"fixture.overflow"});
        assert_eq!(assess(&Known, &o, &d, true, false), Err(Error::Limit));
    }
}
