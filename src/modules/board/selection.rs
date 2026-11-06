//! Immutable selected terms and role derivation, with no quote acceptance or payment authority.
use super::{
    Error,
    authority::Action,
    cache::{Cache, Provenance, Remote},
    contract::{Context, Snapshot},
    definition::Definition,
    offering::{Direction, Offering},
    semantics::{self, Capabilities},
    state::Provider,
};
use crate::modules::identity::api::IdentityId;

/// Explicit caller requirements; array order and TCP direction supply no implicit selection policy.
pub struct Request<'a> {
    /// Exact offering ID chosen by the application.
    pub id: &'a str,
    /// Exact service identifier expected by the caller.
    pub service: &'a str,
    /// Exact supported version, with no nearby-version substitution.
    pub version: u32,
    /// Expected observable contract description.
    pub description: &'a str,
    /// The identity participating opposite the Board publisher.
    pub other: IdentityId,
    /// Trusted local authentication observation, not a peer JSON field.
    pub authenticated: bool,
}
/// Owned immutable terms to copy into a future accepted invocation context.
#[derive(Clone)]
pub struct Selection {
    pub(super) offering: Offering,
    pub(super) definition: Definition,
    pub(super) publisher: IdentityId,
    requester: IdentityId,
    provider: IdentityId,
}
impl Selection {
    /// Borrow immutable economic and descriptive terms; the caller gains no permission to execute.
    pub fn offering(&self) -> &Offering {
        tracing::trace!("Reading immutable selected offering terms");
        &self.offering
    }
    /// Borrow the exact validated definition associated with these terms.
    pub fn definition(&self) -> &Definition {
        tracing::trace!("Reading immutable selected definition");
        &self.definition
    }
    /// Return requester, provider and issuer independent of stream or TCP opener.
    pub fn roles(&self) -> (IdentityId, IdentityId, IdentityId) {
        tracing::trace!("Reading explicit work and payment roles");
        (self.requester, self.provider, self.offering.issuer)
    }
    /// Return the publisher bound to this selection.
    pub fn publisher(&self) -> IdentityId {
        tracing::trace!("Reading selection publisher identity");
        self.publisher
    }
    /// Conservatively compare logical contracts; matching terms still require explicit economic policy.
    pub fn contract_matches(&self, other: &Self) -> bool {
        tracing::trace!("Comparing selected contracts without accepting terms");
        self.definition.same_contract(&other.definition)
    }
}
// Validate exact requested contract, required semantics and direction-derived roles before returning owned terms.
fn select(
    o: &Offering,
    d: &Definition,
    publisher: IdentityId,
    r: &Request<'_>,
    caps: &dyn Capabilities,
) -> Result<Selection, Error> {
    tracing::trace!("Selecting one explicit offering without executing or spending");
    if o.service != r.service || o.version != r.version || o.description != r.description {
        return Err(Error::Stale);
    }
    semantics::assess(caps, o, d, r.authenticated, false)?;
    let (requester, provider) = match o.direction {
        Direction::Provided => (r.other, publisher),
        Direction::Wanted => (publisher, r.other),
    };
    Ok(Selection {
        offering: o.clone(),
        definition: d.clone(),
        publisher,
        requester,
        provider,
    })
}
impl Provider {
    /// Select one exact local pinned offering under Select authority.
    /// # Errors
    /// Rejects foreign/missing/stale contracts, unsupported semantics and caller control failures.
    pub fn select(
        &self,
        context: &Context<'_>,
        snapshot: &Snapshot,
        r: &Request<'_>,
    ) -> Result<Selection, Error> {
        tracing::trace!("Selecting terms from a local publication");
        self.check_snapshot(snapshot)?;
        context.check(
            &self.authority,
            self.publisher,
            Action::Select,
            snapshot.bytes().len(),
        )?;
        let data = snapshot.data.as_ref().ok_or(Error::Missing)?;
        select(
            data.board.offerings.get(r.id).ok_or(Error::Missing)?,
            data.definitions.get(r.id).ok_or(Error::Missing)?,
            self.publisher,
            r,
            self.caps.as_ref(),
        )
    }
}
impl Cache {
    /// Select from a fresh directly authenticated receipt; forwarding alone cannot authenticate a publisher.
    /// # Errors
    /// Rejects forwarded, stale, missing or unsupported selections and authority failures.
    pub fn select(
        &self,
        context: &Context<'_>,
        remote: &Remote,
        r: &Request<'_>,
        caps: &dyn Capabilities,
        now: u64,
    ) -> Result<Selection, Error> {
        tracing::trace!("Selecting attributed remote Board terms");
        self.current(remote, now)?;
        let Provenance::Direct(publisher) = remote.provenance else {
            return Err(Error::Unsupported);
        };
        context.check(
            &self.authority,
            publisher,
            Action::Select,
            remote.bytes().len(),
        )?;
        select(
            remote.board.offerings.get(r.id).ok_or(Error::Missing)?,
            remote.definitions.get(r.id).ok_or(Error::Missing)?,
            publisher,
            r,
            caps,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::board::{config::Limits, offering::Board, test_support};
    struct Known;
    impl Capabilities for Known {
        // The fixture explicitly understands exactly one custom contract.
        fn understands(&self, o: &Offering) -> bool {
            o.service == "org.example.echo" && o.version == 1
        }
        // Only fixed-price fixtures are supported by this test model.
        fn terms(&self, o: &Offering) -> Result<(), Error> {
            if o.source.get("price").is_some() {
                Ok(())
            } else {
                Err(Error::Unsupported)
            }
        }
        // Unknown preconditions never become satisfied by default.
        fn precondition(&self, n: &str, a: bool) -> bool {
            n == "cr2se.identity" && a
        }
        // Test-owned requester/provider capability is explicit, never installed by the application.
        fn local(&self, _: &Offering) -> bool {
            true
        }
    }
    // Work roles follow provided/wanted publication while the issuer remains the exact explicit identity.
    #[test]
    fn work_roles_and_contract_selection() {
        let publisher = IdentityId([1; 32]);
        let other = IdentityId([2; 32]);
        for wanted in [false, true] {
            let b =
                Board::parse(test_support::board(wanted, 7).as_bytes(), Limits::default()).unwrap();
            let o = &b.offerings["fixture"];
            let d = Definition::parse(test_support::definition().as_bytes(), o, Limits::default())
                .unwrap();
            let mut r = Request {
                id: "fixture",
                service: "org.example.echo",
                version: 1,
                description: "Return the supplied label.",
                other,
                authenticated: true,
            };
            let s = select(o, &d, publisher, &r, &Known).unwrap();
            assert_eq!(
                s.roles(),
                if wanted {
                    (publisher, other, o.issuer)
                } else {
                    (other, publisher, o.issuer)
                }
            );
            assert_eq!(s.publisher(), publisher);
            assert_eq!(s.offering().id, "fixture");
            assert_eq!(
                s.definition().document.bytes(),
                test_support::definition().as_bytes()
            );
            assert!(s.contract_matches(&s));
            r.version = 2;
            assert!(matches!(
                select(o, &d, publisher, &r, &Known),
                Err(Error::Stale)
            ));
        }
    }
}
