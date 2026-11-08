//! Identity-bound durable coordinator images; uncertainty gates writes until authoritative reload.
use super::{
    Error,
    authority::{Action, Authority},
    contract::{Context, Limits},
    persistence::{Persistence, Record},
};
use crate::modules::identity::api::IdentityId;
use std::sync::{Arc, Mutex};

struct State<P> {
    store: P,
    payload: Vec<u8>,
    uncertain: bool,
}
/// Single-writer journal for coordinator-owned bounded bytes, separate from paid Storage.
/// Runtime codecs must validate the private operation schema before serving recovered work.
pub struct Journal<P: Persistence> {
    local: IdentityId,
    authority: Arc<Authority>,
    limits: Limits,
    state: Mutex<State<P>>,
}
impl<P: Persistence> Journal<P> {
    /// Open existing complete state or explicitly initialize absent state with a validated image.
    /// Initialization requires Register; existing reads require Recover. Never resets corrupt/wrong-identity state.
    /// # Errors
    /// Rejects missing existing state, wrong identity, invalid limits, authority and storage failures.
    pub fn open(
        context: &Context<'_>,
        local: IdentityId,
        authority: Arc<Authority>,
        limits: Limits,
        mut store: P,
        initial: Option<Vec<u8>>,
    ) -> Result<Self, Error> {
        tracing::trace!("Opening authoritative Services coordinator journal");
        limits.validate()?;
        let action = if initial.is_some() {
            Action::Register
        } else {
            Action::Recover
        };
        context.check(
            &authority,
            local,
            action,
            initial.as_ref().map_or(0, Vec::len),
        )?;
        let payload = match store.load()? {
            Some(record) => {
                if initial.is_some() {
                    return Err(Error::Conflict);
                }
                if record.identity != local {
                    return Err(Error::Denied);
                }
                record.payload
            }
            None => {
                let payload = initial.ok_or(Error::Unavailable)?;
                if payload.len() > limits.state_bytes {
                    return Err(Error::Limit);
                }
                store.commit(&Record {
                    identity: local,
                    payload: payload.clone(),
                })?;
                payload
            }
        };
        if payload.len() > limits.state_bytes {
            return Err(Error::Limit);
        }
        Ok(Self {
            local,
            authority,
            limits,
            state: Mutex::new(State {
                store,
                payload,
                uncertain: false,
            }),
        })
    }
    /// Return an immutable complete recovered image under observation, coordination or recovery authority.
    /// # Errors
    /// Unknown gates observations after possible writes; callers choose the exact expected action.
    pub fn read(&self, context: &Context<'_>, action: Action) -> Result<Vec<u8>, Error> {
        tracing::trace!("Reading complete coordinator journal image");
        if !matches!(
            action,
            Action::Observe | Action::Coordinate | Action::Recover
        ) {
            return Err(Error::Denied);
        }
        let state = self.state.lock().map_err(|_| Error::Unavailable)?;
        context.check(&self.authority, self.local, action, state.payload.len())?;
        if state.uncertain {
            return Err(Error::Unknown);
        }
        Ok(state.payload.clone())
    }
    /// Compare and replace one complete image durably under coordinator authority.
    /// The compare prevents two concurrent coordinators from discarding each other's operation records.
    /// # Errors
    /// Conflict rejects stale images; Unknown gates dependent work until recover; Storage preserves old visibility.
    pub fn replace(
        &self,
        context: &Context<'_>,
        expected: &[u8],
        payload: Vec<u8>,
    ) -> Result<(), Error> {
        tracing::trace!("Durably replacing an exact coordinator journal image");
        if payload.len() > self.limits.state_bytes {
            return Err(Error::Limit);
        }
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        context.check(
            &self.authority,
            self.local,
            Action::Coordinate,
            payload.len(),
        )?;
        if state.uncertain {
            return Err(Error::Unknown);
        }
        if state.payload != expected {
            return Err(Error::Conflict);
        }
        match state.store.commit(&Record {
            identity: self.local,
            payload: payload.clone(),
        }) {
            Ok(()) => {
                state.payload = payload;
                Ok(())
            }
            Err(Error::Unknown) => {
                state.uncertain = true;
                Err(Error::Unknown)
            }
            Err(e) => Err(e),
        }
    }
    /// Reload and synchronize authoritative bytes without repeating the possibly committed write.
    /// # Errors
    /// Refuses missing, oversized or wrong-identity recovered state; uncertainty stays gated on failure.
    pub fn recover(&self, context: &Context<'_>) -> Result<Vec<u8>, Error> {
        tracing::trace!("Recovering authoritative coordinator bytes without replaying mutation");
        let mut state = self.state.lock().map_err(|_| Error::Unavailable)?;
        context.check(&self.authority, self.local, Action::Recover, 0)?;
        state.uncertain = true;
        let record = state.store.load()?.ok_or(Error::Unknown)?;
        if record.identity != self.local {
            return Err(Error::Denied);
        }
        if record.payload.len() > self.limits.state_bytes {
            return Err(Error::Limit);
        }
        context.check(
            &self.authority,
            self.local,
            Action::Recover,
            record.payload.len(),
        )?;
        state.payload = record.payload;
        state.uncertain = false;
        Ok(state.payload.clone())
    }
}
