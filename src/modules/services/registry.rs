//! Authorized exact-contract registration and Board capability adapter; disabled entries remain recoverable.
use super::{
    Error,
    authority::{Action, Authority},
    contract::{Context, Limits},
    handler::{Provider, Requester},
};
use crate::modules::{
    board::{
        self,
        definition::Definition,
        offering::{Direction, Offering},
        semantics::Capabilities,
    },
    identity::api::IdentityId,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

/// Independently trusted standard-contract catalogue supplied by application composition.
pub trait Standards: Send + Sync {
    /// Check a complete registered definition against the authoritative standard contract.
    /// # Errors
    /// Absent or changed standard contracts must be Unsupported or Invalid.
    fn validate(&self, definition: &Definition) -> Result<(), Error>;
}
/// Conservative composition without any installed standard family.
pub struct NoStandards;
impl Standards for NoStandards {
    // Names in the reserved namespace do not authorize a standard provider by themselves.
    fn validate(&self, _: &Definition) -> Result<(), Error> {
        tracing::trace!("Refusing an absent standard contract");
        Err(Error::Unsupported)
    }
}
/// Retained implementation pin; draining registration cannot invalidate accepted work.
#[derive(Clone)]
pub struct Registered {
    pub(crate) requester: Arc<dyn Requester>,
    pub(crate) provider: Option<Arc<dyn Provider>>,
}
impl Registered {
    /// Borrow the retained requester codec and verifier.
    pub fn requester(&self) -> &dyn Requester {
        tracing::trace!("Borrowing pinned requester implementation");
        self.requester.as_ref()
    }
    /// Borrow the retained provider, if one was explicitly installed.
    /// # Errors
    /// Requester-only registrations return Unsupported.
    pub fn provider(&self) -> Result<&dyn Provider, Error> {
        tracing::trace!("Borrowing pinned provider implementation");
        self.provider.as_deref().ok_or(Error::Unsupported)
    }
}
struct Entry {
    registered: Registered,
    active: bool,
}
/// Registry owned by one local identity, independent of peer descriptions and transport direction.
pub struct Registry {
    authority: Arc<Authority>,
    local: IdentityId,
    limits: Limits,
    standards: Arc<dyn Standards>,
    entries: RwLock<BTreeMap<(String, u32), Entry>>,
}
impl Registry {
    /// Create a finite empty registry; standard families require an independent trusted catalogue.
    /// # Errors
    /// Rejects invalid finite limits.
    pub fn new(
        local: IdentityId,
        authority: Arc<Authority>,
        limits: Limits,
        standards: Arc<dyn Standards>,
    ) -> Result<Self, Error> {
        tracing::trace!("Creating an empty authorized Services registry");
        limits.validate()?;
        Ok(Self {
            local,
            authority,
            limits,
            standards,
            entries: RwLock::new(BTreeMap::new()),
        })
    }
    /// Install one exact requester contract and optional provider under registration authority.
    /// # Errors
    /// Denies foreign authority; rejects malformed, unsupported or duplicate definitions and capacity overflow.
    pub fn register(
        &self,
        context: &Context<'_>,
        requester: Arc<dyn Requester>,
        provider: Option<Arc<dyn Provider>>,
    ) -> Result<(), Error> {
        tracing::trace!("Registering an explicitly installed service implementation");
        let d = requester.definition();
        context.check(
            &self.authority,
            self.local,
            Action::Register,
            d.document.bytes().len(),
        )?;
        let v = d.document.value();
        let service = v["service"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or(Error::Invalid)?
            .to_owned();
        let version = v["serviceVersion"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n > 0)
            .ok_or(Error::Invalid)?;
        if d.support != super::schema::Support::Known {
            return Err(Error::Unsupported);
        }
        if service.starts_with("cr2se.") {
            self.standards.validate(d)?;
        }
        let mut entries = self.entries.write().map_err(|_| Error::Unavailable)?;
        context.check(
            &self.authority,
            self.local,
            Action::Register,
            d.document.bytes().len(),
        )?;
        if entries.contains_key(&(service.clone(), version)) {
            return Err(Error::Conflict);
        }
        if entries.len() >= self.limits.handlers {
            return Err(Error::Limit);
        }
        entries.insert(
            (service, version),
            Entry {
                registered: Registered {
                    requester,
                    provider,
                },
                active: true,
            },
        );
        Ok(())
    }
    /// Pin an active exact contract and optionally require actual provider support.
    /// # Errors
    /// Missing, disabled or provider-less capabilities return Unsupported without fallback.
    pub fn select(&self, service: &str, version: u32, provider: bool) -> Result<Registered, Error> {
        tracing::trace!("Selecting an exact active service registration");
        let entries = self.entries.read().map_err(|_| Error::Unavailable)?;
        let e = entries
            .get(&(service.to_owned(), version))
            .filter(|e| e.active)
            .ok_or(Error::Unsupported)?;
        if provider && e.registered.provider.is_none() {
            return Err(Error::Unsupported);
        }
        Ok(e.registered.clone())
    }
    /// Disable new selection while retaining implementation and recovery obligations.
    /// # Errors
    /// Denies foreign registration authority and missing exact contracts.
    pub fn drain(&self, context: &Context<'_>, service: &str, version: u32) -> Result<(), Error> {
        tracing::trace!("Draining service registration without discarding accepted work");
        context.check(&self.authority, self.local, Action::Register, service.len())?;
        let mut entries = self.entries.write().map_err(|_| Error::Unavailable)?;
        context.check(&self.authority, self.local, Action::Register, service.len())?;
        entries
            .get_mut(&(service.to_owned(), version))
            .ok_or(Error::Unsupported)?
            .active = false;
        Ok(())
    }
    /// Recover an exact retained implementation, including drained entries, under recovery authority.
    /// # Errors
    /// Missing recovery implementations gate affected work; authority is checked after acquiring the lock.
    pub fn recover(
        &self,
        context: &Context<'_>,
        service: &str,
        version: u32,
    ) -> Result<Registered, Error> {
        tracing::trace!("Recovering retained service implementation");
        let entries = self.entries.read().map_err(|_| Error::Unavailable)?;
        context.check(&self.authority, self.local, Action::Recover, service.len())?;
        entries
            .get(&(service.to_owned(), version))
            .map(|e| e.registered.clone())
            .ok_or(Error::Unavailable)
    }
}
// Preserve structured local failures when serving the Board capability contract.
fn board_error(error: Error) -> board::Error {
    match error {
        Error::Invalid => board::Error::Invalid,
        Error::Limit => board::Error::Limit,
        Error::Denied => board::Error::Denied,
        Error::Conflict => board::Error::Stale,
        Error::Cancelled => board::Error::Cancelled,
        Error::Storage => board::Error::Storage,
        Error::Unknown => board::Error::Unknown,
        _ => board::Error::Unsupported,
    }
}
impl Capabilities for Registry {
    // Understanding requires an active exact requester contract, never nearby versions.
    fn understands(&self, o: &Offering) -> bool {
        tracing::trace!("Checking registered exact Board service semantics");
        self.select(&o.service, o.version, false).is_ok()
    }
    // Compare complete logical definitions, including nested schemas and unknown metadata.
    fn contract(&self, o: &Offering, d: &Definition) -> Result<(), board::Error> {
        tracing::trace!("Checking Board definition against installed contract");
        let r = self
            .select(&o.service, o.version, false)
            .map_err(board_error)?;
        if !r.requester.definition().same_contract(d) {
            return Err(board::Error::Unsupported);
        }
        Ok(())
    }
    // The installed family independently validates economic models without execution or spending.
    fn terms(&self, o: &Offering) -> Result<(), board::Error> {
        tracing::trace!("Validating family Board economic terms");
        self.select(&o.service, o.version, false)
            .map_err(board_error)?
            .requester
            .terms(o)
            .map_err(board_error)
    }
    // Only the authenticated identity precondition is built in; other conditions need future explicit adapters.
    fn precondition(&self, name: &str, authenticated: bool) -> bool {
        tracing::trace!("Checking understood identity precondition");
        name == "cr2se.identity" && authenticated
    }
    // Provided work requires a real provider; wanted work requires an independently registered requester codec.
    fn local(&self, o: &Offering) -> bool {
        tracing::trace!("Checking provided or wanted implementation availability");
        self.select(&o.service, o.version, o.direction == Direction::Provided)
            .is_ok()
    }
}
