//! Direct authenticated metadata cache and independent current-contract checks for outgoing work.
use super::*;
use std::sync::Mutex;
/// Volatile bounded cache; only normally completed original-peer metadata calls can populate it.
pub struct PeerCache {
    state: Mutex<board::cache::Cache>,
    authority: Arc<board::authority::Authority>,
    registry: Arc<services::registry::Registry>,
    limits: board::config::Limits,
    epoch: Instant,
}
impl PeerCache {
    /// Create an empty cache over independently installed requester contracts, without fetching or trusting peer documents.
    /// # Errors
    /// Invalid Board resource/expiry limits fail before a cache is created.
    pub fn new(
        registry: Arc<services::registry::Registry>,
        limits: board::config::Limits,
    ) -> Result<Self, Error> {
        tracing::trace!("Creating a bounded direct-publisher metadata cache");
        let authority = Arc::new(board::authority::Authority::new());
        Ok(Self {
            state: Mutex::new(board::cache::Cache::new(authority.clone(), limits)?),
            authority,
            registry,
            limits,
            epoch: Instant::now(),
        })
    }
    /// Select a fresh directly authenticated offering using the locally installed contract description.
    /// # Errors
    /// Missing/forwarded/expired definitions, unsupported contracts or changed role bindings cannot produce a selection.
    pub fn selection(
        &self,
        publisher: IdentityId,
        offering: &str,
        service: &str,
        version: u32,
    ) -> Result<board::selection::Selection, Error> {
        tracing::trace!("Selecting fresh direct metadata against installed requester semantics");
        self.select(
            publisher,
            offering,
            service,
            version,
            Instant::now()
                .checked_add(Duration::from_secs(5))
                .ok_or(Error::Limit)?,
        )
    }
    // Read a fresh sealed receipt, preserving publisher attribution independently of the current connection.
    pub(super) fn receipt(
        &self,
        publisher: IdentityId,
        deadline: Instant,
    ) -> Result<board::cache::Remote, Error> {
        tracing::trace!("Reading an original directly authenticated metadata receipt");
        let grant = self.authority.grant(
            publisher,
            board::authority::Action::Read,
            self.limits.publication_bytes,
        )?;
        Ok(self.state.lock().map_err(|_| Error::Unavailable)?.read(
            &board_context(&grant, deadline),
            board::cache::Provenance::Direct(publisher),
            self.now()?,
        )?)
    }
    // Store exact metadata bytes only through the request's original normal-completion/provenance gate.
    pub(super) fn apply(
        &self,
        request: peer::metadata_client::Request,
        context: &Context<'_>,
        association: Association,
        response: &Message,
    ) -> Result<board::cache::Remote, Error> {
        tracing::trace!("Applying complete original-peer metadata through cache authority");
        if association.local != self.registry.local_identity() {
            return Err(Error::Denied);
        }
        let grant = self.authority.grant(
            association.peer,
            board::authority::Action::Cache,
            self.limits.publication_bytes,
        )?;
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        request.apply(
            context,
            association,
            response,
            &mut state,
            &board_context(&grant, context.deadline),
            self.now()?,
        )
    }
    // Revalidate direct cache freshness and installed contract before accepting a peer quote.
    pub(super) fn current(self: &Arc<Self>, deadline: Instant) -> Current {
        tracing::trace!("Binding a requester current-contract view to its original deadline");
        Current {
            cache: self.clone(),
            deadline,
        }
    }
    // Select with the original call deadline and whole monotonic seconds expected by Board's cache contract.
    fn select(
        &self,
        publisher: IdentityId,
        offering: &str,
        service: &str,
        version: u32,
        deadline: Instant,
    ) -> Result<board::selection::Selection, Error> {
        tracing::trace!("Rechecking directly cached current offering contracts");
        let installed = self.registry.select(service, version, false)?;
        let definition = installed.requester().definition();
        let description = definition.document.value()["description"]
            .as_str()
            .ok_or(Error::Invalid)?;
        let read = self.authority.grant(
            publisher,
            board::authority::Action::Read,
            self.limits.publication_bytes,
        )?;
        let select = self.authority.grant(
            publisher,
            board::authority::Action::Select,
            self.limits.publication_bytes,
        )?;
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        let now = self.now()?;
        let receipt = state.read(
            &board_context(&read, deadline),
            board::cache::Provenance::Direct(publisher),
            now,
        )?;
        Ok(state.select(
            &board_context(&select, deadline),
            &receipt,
            &board::selection::Request {
                id: offering,
                service,
                version,
                description,
                other: self.registry.local_identity(),
                authenticated: true,
            },
            self.registry.as_ref(),
            now,
        )?)
    }
    // Cache age is process-local monotonic seconds; a restart starts with an empty cache.
    fn now(&self) -> Result<u64, Error> {
        tracing::trace!("Reading local cache age in whole seconds");
        Instant::now()
            .checked_duration_since(self.epoch)
            .map(|elapsed| elapsed.as_secs())
            .ok_or(Error::Unknown)
    }
}
/// Original-call current-contract view; economic quote changes still require independent validation and explicit consent.
pub(super) struct Current {
    cache: Arc<PeerCache>,
    deadline: Instant,
}
impl services::quote::Current for Current {
    // Fresh cache and installed definitions must retain identities, direction and contract; quote economics are separately consented.
    fn check(&self, selected: &board::selection::Selection) -> Result<(), services::Error> {
        tracing::trace!(
            "Checking requester current contract without inventing publisher authority"
        );
        let check = || -> Result<(), Error> {
            let o = selected.offering();
            let current = self.cache.select(
                selected.publisher(),
                &o.id,
                &o.service,
                o.version,
                self.deadline,
            )?;
            if !current.contract_matches(selected)
                || current.roles() != selected.roles()
                || current.offering().direction != o.direction
            {
                return Err(Error::Conflict);
            }
            Ok(())
        };
        check().map_err(|e| match e {
            Error::Invalid => services::Error::Invalid,
            Error::Unsupported => services::Error::Unsupported,
            Error::Denied => services::Error::Denied,
            Error::Limit => services::Error::Limit,
            Error::Cancelled => services::Error::Cancelled,
            Error::Conflict => services::Error::Conflict,
            Error::Insufficient => services::Error::Insufficient,
            Error::Storage => services::Error::Storage,
            Error::Unknown => services::Error::Unknown,
            Error::Unavailable => services::Error::Unavailable,
        })
    }
}
