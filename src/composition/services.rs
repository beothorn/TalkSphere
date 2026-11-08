//! Serialize publication and handler lifecycle through public Board/Services contracts.
use crate::modules::identity::api::IdentityId;
use crate::modules::{
    board::{
        self,
        contract::{Context as BoardContext, Snapshot},
        prepared::Prepared,
        selection::{Request, Selection},
        state::Provider,
    },
    services::{Error, contract::Context, registry::Registry},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

/// Composition-owned publication facade; keep all publication writers behind this gate.
pub struct Publication {
    local: IdentityId,
    owner: Provider,
    registry: Arc<Registry>,
    limits: board::config::Limits,
    gate: Mutex<()>,
}
impl Publication {
    /// Retain exclusive application ownership of a Board and its matching local Services registry.
    /// # Errors
    /// Rejects invalid limits or mismatching registry identity before publication effects.
    pub fn new(
        local: IdentityId,
        owner: Provider,
        registry: Arc<Registry>,
        limits: board::config::Limits,
    ) -> Result<Self, Error> {
        tracing::trace!("Composing serialized service publication lifecycle");
        limits.validate().map_err(board_error)?;
        if registry.local_identity() != local {
            return Err(Error::Denied);
        }
        Ok(Self {
            local,
            owner,
            registry,
            limits,
            gate: Mutex::new(()),
        })
    }
    /// Publish only capabilities currently supported by this facade's actual registry, under one composition writer.
    /// # Errors
    /// Rejects foreign identity, unavailable semantics, caller failures and uncertain storage outcomes.
    pub fn publish(
        &self,
        context: &BoardContext<'_>,
        candidate: Prepared,
    ) -> Result<Snapshot, Error> {
        tracing::trace!("Publishing registry-validated service capabilities");
        let _guard = self.gate.lock().map_err(|_| Error::Unavailable)?;
        if candidate.publisher() != self.local {
            return Err(Error::Denied);
        }
        candidate
            .assess(self.registry.as_ref())
            .map_err(board_error)?;
        self.owner.publish(context, candidate).map_err(board_error)
    }
    /// Read an immutable complete current publication without exposing another owner's state.
    /// # Errors
    /// Authority, missing publication and uncertain storage failures propagate.
    pub fn read(&self, context: &BoardContext<'_>) -> Result<Snapshot, Error> {
        tracing::trace!("Reading composition-owned service publication");
        let snapshot = self.owner.read(context).map_err(board_error)?;
        if snapshot.publisher() != self.local {
            return Err(Error::Denied);
        }
        Ok(snapshot)
    }
    /// Select exact current terms through the Board's own public role/capability contract.
    /// # Errors
    /// Stale, missing, unsupported or unauthorized selection fails explicitly.
    pub fn select(
        &self,
        context: &BoardContext<'_>,
        snapshot: &Snapshot,
        request: &Request<'_>,
    ) -> Result<Selection, Error> {
        tracing::trace!("Selecting composition-owned current service terms");
        self.owner
            .select(context, snapshot, request)
            .map_err(board_error)
    }
    /// Remove matching offerings durably before disabling new registry selection; retained work remains pinned.
    /// # Errors
    /// Publication/authority uncertainty stops draining; no concurrent facade writer can be overwritten.
    pub fn drain(
        &self,
        context: &Context<'_>,
        read: &BoardContext<'_>,
        publish: &BoardContext<'_>,
        service: &str,
        version: u32,
    ) -> Result<(), Error> {
        tracing::trace!("Unpublishing service before draining new handler selection");
        self.registry.validate_update(context, service.len())?;
        let _guard = self.gate.lock().map_err(|_| Error::Unavailable)?;
        let snapshot = match self.owner.read(read) {
            Ok(snapshot) => snapshot,
            Err(board::Error::Missing) => return self.registry.drain(context, service, version),
            Err(e) => return Err(board_error(e)),
        };
        if snapshot.publisher() != self.local {
            return Err(Error::Denied);
        }
        let board =
            board::offering::Board::parse(snapshot.bytes(), self.limits).map_err(board_error)?;
        let mut value = board.document.value().clone();
        for field in ["providedServices", "wantedServices"] {
            value[field]
                .as_array_mut()
                .ok_or(Error::Invalid)?
                .retain(|o| {
                    o["service"].as_str() != Some(service)
                        || board::offering::integer(&o["serviceVersion"])
                            != Some(u64::from(version))
                });
        }
        let mut definitions = BTreeMap::new();
        for (id, offering) in &board.offerings {
            if offering.service != service || offering.version != version {
                let definition = self
                    .owner
                    .definition(read, &snapshot, id)
                    .map_err(board_error)?;
                definitions.insert(id.clone(), definition.document.bytes().to_vec());
            }
        }
        let candidate = Prepared::new(
            self.local,
            value.to_string().as_bytes(),
            &definitions,
            self.limits,
            self.registry.as_ref(),
        )
        .map_err(board_error)?;
        self.owner
            .publish(publish, candidate)
            .map_err(board_error)?;
        self.registry.drain(context, service, version)
    }
    /// Resolve a possibly committed publication before retrying lifecycle changes; no mutation is replayed.
    /// # Errors
    /// Failed recovery leaves Board availability gated rather than disabling an advertised handler.
    pub fn recover(&self, context: &BoardContext<'_>) -> Result<(), Error> {
        tracing::trace!("Recovering service publication lifecycle through Board owner");
        let _guard = self.gate.lock().map_err(|_| Error::Unavailable)?;
        self.owner.recover(context).map_err(board_error)
    }
    /// Compare immutable terms with the current publication at explicit quote acceptance.
    /// # Errors
    /// Removed or changed offerings conflict; authority and uncertainty remain distinct.
    pub fn current(&self, context: &BoardContext<'_>, selection: &Selection) -> Result<(), Error> {
        tracing::trace!("Checking current composition-owned offering before acceptance");
        self.owner
            .current_selection(context, selection)
            .map_err(board_error)
    }
}
// Preserve structured module outcomes rather than making dependency loss look like peer rejection.
fn board_error(error: board::Error) -> Error {
    match error {
        board::Error::Invalid => Error::Invalid,
        board::Error::Unsupported | board::Error::Missing => Error::Unsupported,
        board::Error::Stale => Error::Conflict,
        board::Error::Denied => Error::Denied,
        board::Error::Limit => Error::Limit,
        board::Error::Cancelled => Error::Cancelled,
        board::Error::Storage => Error::Storage,
        board::Error::Unknown => Error::Unknown,
    }
}
