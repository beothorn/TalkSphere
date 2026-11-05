//! One-shot configured contact attempts retain expected identity and never create a reconnect policy.
use super::{Failure, Stage, transport::Transport};
use crate::modules::transport::{
    api::{Accepted, ConnectRequest, ExpectedPeer, Request, TransportControl},
    handles::ConnectionHandle,
    ports::identity::IdentityId,
};
/// Immediate admission outcome for one configured contact; successful admission is not authentication.
#[derive(Debug)]
pub struct Attempt {
    /// Pinned public identity required by this connection's handshake.
    pub expected_id: IdentityId,
    /// Local connection authority or typed setup failure; later results arrive as driver events.
    pub admitted: Result<ConnectionHandle, Failure>,
}
impl Transport {
    /// Start each configured endpoint at most once, retaining accepted connection authority.
    /// # Errors
    /// Repeated calls or calls outside Ready return Runtime; individual failures remain in the result list.
    pub fn connect_contacts(&mut self) -> Result<Vec<Attempt>, Failure> {
        tracing::trace!("Starting bounded one-shot configured contact attempts");
        if self.stage != Stage::Ready || self.contacts_started {
            return Err(Failure::Runtime);
        }
        self.contacts_started = true;
        let mut outcomes = Vec::with_capacity(self.contacts.len());
        for contact in &self.contacts {
            let expected_id = IdentityId(contact.expected_id.0);
            let admitted = contact
                .endpoint
                .ok_or(Failure::Network)
                .and_then(|endpoint| {
                    ConnectRequest::new(endpoint, ExpectedPeer::Known(expected_id))
                        .map_err(|_| Failure::Network)
                })
                .and_then(
                    |request| match self.driver.submit(Request::Connect(request)) {
                        Ok(Accepted::Connection(handle)) => Ok(handle),
                        _ => Err(Failure::Network),
                    },
                );
            if let Ok(handle) = &admitted {
                self.connections.push(handle.clone());
            }
            outcomes.push(Attempt {
                expected_id,
                admitted,
            });
        }
        Ok(outcomes)
    }
    /// Inspect retained local connection authority count without exposing private identity material.
    pub fn connection_count(&self) -> usize {
        tracing::trace!("Inspecting retained server connection count");
        self.connections.len()
    }
}
