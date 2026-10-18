//! In-process capabilities, never reconstructed from peer IDs or wire bytes.

use std::{fmt, num::NonZeroU32, sync::Arc};

use super::TransportError;

#[derive(Clone)]
struct Instance(Arc<()>);

impl PartialEq for Instance {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Instance {}

impl fmt::Debug for Instance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<opaque>")
    }
}

/// Observable connection identity, carrying no connection-control authority.
///
/// Equality identifies the same live instance, not the same peer identity.
/// IDs cannot be serialized, persisted or reused after process restart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionId(Instance);

/// Local authority to open, ping, drain or close one connection.
/// Cloning deliberately delegates the same authority.
///
/// Peers and consumers cannot mint a handle from an observed connection ID.
/// ```compile_fail
/// use talksphere::modules::transport::handles::{ConnectionHandle, ConnectionId};
/// fn forge(id: ConnectionId) -> ConnectionHandle {
///     ConnectionHandle { id }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionHandle {
    id: ConnectionId,
}

impl ConnectionHandle {
    // Issue a fresh instance capability for admitted local composition.
    pub(crate) fn new() -> Self {
        tracing::trace!(
            function = "ConnectionHandle::new",
            "Issuing connection authority"
        );
        Self {
            id: ConnectionId(Instance(Arc::new(()))),
        }
    }

    pub fn id(&self) -> &ConnectionId {
        &self.id
    }
}

/// Local authority for a listener; never a peer-provided socket identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListenerHandle(Instance);

/// Authority issued to the consumer when a protocol registration succeeds.
/// Sharing a clone deliberately delegates ownership of its streams.
/// Public peer identity bytes cannot be used as local owner authority.
/// ```compile_fail
/// use talksphere::modules::transport::{
///     handles::StreamOwner, ports::identity::IdentityId,
/// };
/// let owner = StreamOwner::from(IdentityId([0; 32]));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamOwner(Instance);

impl StreamOwner {
    // Issue authority only after successful local protocol registration.
    pub(crate) fn new() -> Self {
        tracing::trace!(
            function = "StreamOwner::new",
            "Issuing protocol owner authority"
        );
        Self(Instance(Arc::new(())))
    }
}

/// A stream scoped to both a connection instance and its registered owner.
/// Inspecting this handle does not grant connection-control authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamHandle {
    connection: ConnectionId,
    owner: StreamOwner,
    wire_id: NonZeroU32,
}

impl StreamHandle {
    // Bind a nonzero wire ID to its connection and authorized consumer.
    pub(crate) fn new(
        connection: &ConnectionHandle,
        owner: &StreamOwner,
        wire_id: NonZeroU32,
    ) -> Self {
        tracing::trace!(
            function = "StreamHandle::new",
            "Issuing scoped stream authority"
        );
        Self {
            connection: connection.id.clone(),
            owner: owner.clone(),
            wire_id,
        }
    }

    pub fn connection_id(&self) -> &ConnectionId {
        &self.connection
    }

    /// Observation only; a number is insufficient to invoke stream operations.
    pub fn wire_id(&self) -> NonZeroU32 {
        self.wire_id
    }

    /// Check caller authority and connection scope before any stream effect.
    /// Runtime liveness and stream state must still be checked by Transport.
    pub fn authorize(
        &self,
        connection: &ConnectionId,
        owner: &StreamOwner,
    ) -> Result<(), TransportError> {
        if self.connection != *connection {
            return Err(TransportError::WrongConnection);
        }
        if self.owner != *owner {
            return Err(TransportError::WrongStreamOwner);
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod test_fixtures {
    use super::*;

    pub fn connection() -> ConnectionHandle {
        ConnectionHandle {
            id: ConnectionId(Instance(Arc::new(()))),
        }
    }

    pub fn owner() -> StreamOwner {
        StreamOwner(Instance(Arc::new(())))
    }

    pub fn stream(connection: &ConnectionHandle, owner: &StreamOwner) -> StreamHandle {
        StreamHandle {
            connection: connection.id.clone(),
            owner: owner.clone(),
            wire_id: NonZeroU32::new(1).unwrap(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{test_fixtures::*, *};

    #[test]
    fn cloned_authority_can_use_its_own_stream() {
        let connection = connection();
        let owner = owner();
        let stream = stream(&connection, &owner);
        assert_eq!(
            stream.authorize(connection.clone().id(), &owner.clone()),
            Ok(())
        );
        assert_eq!(stream.connection_id(), connection.id());
        assert_eq!(stream.wire_id().get(), 1);
    }

    #[test]
    fn another_owner_cannot_use_a_stream_on_the_same_connection() {
        let connection = connection();
        let stream = stream(&connection, &owner());
        assert_eq!(
            stream.authorize(connection.id(), &owner()),
            Err(TransportError::WrongStreamOwner)
        );
    }

    #[test]
    fn same_wire_id_on_another_connection_does_not_grant_access() {
        let first = connection();
        let second = connection();
        let owner = owner();
        let first_stream = stream(&first, &owner);
        let second_stream = stream(&second, &owner);
        assert_eq!(first_stream.wire_id(), second_stream.wire_id());
        assert_ne!(first_stream, second_stream);
        assert_eq!(
            first_stream.authorize(second.id(), &owner),
            Err(TransportError::WrongConnection)
        );
    }

    #[test]
    fn a_new_connection_does_not_revive_an_old_stream() {
        let old_connection = connection();
        let owner = owner();
        let old_stream = stream(&old_connection, &owner);
        drop(old_connection);
        let replacement = connection();
        assert_eq!(
            old_stream.authorize(replacement.id(), &owner),
            Err(TransportError::WrongConnection)
        );
    }
}
