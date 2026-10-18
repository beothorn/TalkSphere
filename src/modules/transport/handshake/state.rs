//! HELLO/AUTH sequencing for one connection, driven by completed frame I/O.
//! No authenticated facts are released until peer verification and local AUTH send.
use super::{SessionKeys, Transcript, derive_keys};
use crate::modules::transport::{
    api::{ConnectionRole, ExpectedPeer},
    codec::{
        FrameHeader, FrameProtection, FrameType,
        handshake::{Auth, HandshakePayloadError, Hello},
    },
    framing::{FramingError, WireFrame},
    ports::{
        DependencyError,
        crypto::Cryptography,
        identity::{Identity, IdentityId},
    },
};

/// Terminal handshake failure; the owning connection must close the socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeError {
    /// Invalid order, duplicate, forbidden frame, or reuse after failure.
    Order,
    /// A well-formed peer connection ERROR; never answer with another ERROR.
    Peer(super::super::codec::control::ErrorCode),
    /// Invalid handshake payload shape or selector.
    Payload(HandshakePayloadError),
    /// Derived identity differs from the configured contact.
    UnexpectedIdentity,
    /// A signing, identity or cryptographic operation failed.
    Dependency(DependencyError),
    /// Outgoing header/payload could not be represented.
    Framing(FramingError),
}

/// Successfully checked connection facts and keys, transferred exactly once.
pub struct Established<K> {
    /// Local TCP role used to bind proofs and keys.
    pub role: ConnectionRole,
    /// Authorized local identity, never derived from a peer claim.
    pub local_identity: IdentityId,
    /// Verified remote identity, including same-identity peers when valid.
    pub peer_identity: IdentityId,
    /// Connection-bound send/receive secrets.
    pub keys: SessionKeys<K>,
}

/// One handshake; frame writing completion must be reported through sent().
pub struct Handshake<C: Cryptography> {
    role: ConnectionRole,
    expected: ExpectedPeer,
    local_identity: IdentityId,
    local: Hello,
    remote: Option<(Hello, IdentityId)>,
    private: Option<C::EphemeralSecret>,
    keys: Option<SessionKeys<C::IntegrityKey>>,
    hello_sent: bool,
    auth_sent: bool,
    peer_verified: bool,
    closed: bool,
}

impl<C: Cryptography> Handshake<C> {
    /// Obtain fresh material and return the first untagged HELLO frame.
    /// # Errors
    /// Propagates credential, entropy, key-generation or local encoding failures.
    pub fn begin(
        role: ConnectionRole,
        expected: ExpectedPeer,
        identity: &impl Identity,
        crypto: &mut C,
    ) -> Result<(Self, WireFrame), HandshakeError> {
        tracing::trace!(
            function = "Handshake::begin",
            "Starting a fresh connection handshake"
        );
        let public = identity
            .public_identity()
            .map_err(HandshakeError::Dependency)?;
        let ephemeral = crypto.ephemeral_key().map_err(HandshakeError::Dependency)?;
        let nonce = crypto.random_nonce().map_err(HandshakeError::Dependency)?;
        let local = Hello {
            identity_public_key: public.signing_public_key,
            ephemeral_public_key: ephemeral.public_key,
            nonce,
        };
        let frame = raw_frame(FrameType::Hello, local.encode().to_vec())?;
        Ok((
            Self {
                role,
                expected,
                local_identity: public.id,
                local,
                remote: None,
                private: Some(ephemeral.private_key),
                keys: None,
                hello_sent: false,
                auth_sent: false,
                peer_verified: false,
                closed: false,
            },
            frame,
        ))
    }
    /// Report a completely written HELLO or AUTH, never mere queue admission.
    /// # Errors
    /// Rejects duplicates, AUTH before HELLO/remote HELLO, and calls after failure.
    pub fn sent(&mut self, kind: FrameType) -> Result<(), HandshakeError> {
        tracing::trace!(
            function = "Handshake::sent",
            "Recording completed handshake output"
        );
        match kind {
            FrameType::Hello if !self.closed && !self.hello_sent => self.hello_sent = true,
            FrameType::Auth
                if !self.closed && self.hello_sent && !self.auth_sent && self.remote.is_some() =>
            {
                self.auth_sent = true
            }
            _ => {
                self.close();
                return Err(HandshakeError::Order);
            }
        }
        Ok(())
    }
    /// Consume one complete handshake frame and optionally produce local AUTH.
    /// # Errors
    /// Every order, payload, contact or primitive failure closes this state and
    /// releases ephemeral/session secrets; it emits no authenticated context.
    pub fn receive(
        &mut self,
        frame: WireFrame,
        identity: &mut impl Identity,
        crypto: &mut C,
    ) -> Result<Option<WireFrame>, HandshakeError> {
        tracing::trace!(
            function = "Handshake::receive",
            "Checking a peer handshake frame"
        );
        let result = self.process(frame, identity, crypto);
        if result.is_err() {
            self.close();
        }
        result
    }
    /// Return direction-specific outbound framing mode after completed local AUTH.
    pub fn send_protection(&self) -> FrameProtection {
        tracing::trace!(
            function = "Handshake::send_protection",
            "Reading outbound handshake phase"
        );
        if self.auth_sent {
            FrameProtection::Tagged
        } else {
            FrameProtection::Untagged
        }
    }
    /// Return inbound mode only after the remote AUTH and derivation succeed.
    pub fn receive_protection(&self) -> FrameProtection {
        tracing::trace!(
            function = "Handshake::receive_protection",
            "Reading inbound handshake phase"
        );
        if self.peer_verified {
            FrameProtection::Tagged
        } else {
            FrameProtection::Untagged
        }
    }
    /// Ordinary application frames require both a sent local AUTH and verified peer.
    pub fn is_authenticated(&self) -> bool {
        tracing::trace!(
            function = "Handshake::is_authenticated",
            "Checking the application dispatch gate"
        );
        !self.closed && self.auth_sent && self.peer_verified
    }
    /// Transfer authenticated facts/keys exactly once, or None before readiness/after closure.
    pub fn take_established(&mut self) -> Option<Established<C::IntegrityKey>> {
        tracing::trace!(
            function = "Handshake::take_established",
            "Transferring authenticated connection facts"
        );
        if !self.is_authenticated() {
            return None;
        }
        let peer_identity = self.remote.as_ref()?.1;
        Some(Established {
            role: self.role,
            local_identity: self.local_identity,
            peer_identity,
            keys: self.keys.take()?,
        })
    }
    /// Explicitly invalidate the handshake and release its secret capabilities.
    pub fn close(&mut self) {
        tracing::trace!(
            function = "Handshake::close",
            "Releasing handshake material"
        );
        self.closed = true;
        self.private = None;
        self.keys = None;
    }
    // Validate order before performing primitive calls or consuming secret capabilities.
    fn process(
        &mut self,
        frame: WireFrame,
        identity: &mut impl Identity,
        crypto: &mut C,
    ) -> Result<Option<WireFrame>, HandshakeError> {
        tracing::trace!(
            function = "Handshake::process",
            "Advancing the ordered handshake"
        );
        if self.closed
            || self.peer_verified
            || frame.header().stream_id() != 0
            || frame.tag().is_some()
        {
            return Err(HandshakeError::Order);
        }
        match frame.header().frame_type() {
            FrameType::Error => {
                let error = super::super::codec::control::ErrorPayload::decode(frame.payload())
                    .map_err(|_| HandshakeError::Order)?;
                Err(HandshakeError::Peer(error.code()))
            }
            FrameType::Hello if self.remote.is_none() => {
                let remote = Hello::decode(frame.payload()).map_err(HandshakeError::Payload)?;
                let id = identity
                    .identify(&remote.identity_public_key)
                    .map_err(HandshakeError::Dependency)?;
                if matches!(self.expected, ExpectedPeer::Known(expected) if expected != id) {
                    return Err(HandshakeError::UnexpectedIdentity);
                }
                let transcript = self.transcript(&remote);
                let auth = transcript
                    .sign(self.role, identity)
                    .map_err(HandshakeError::Dependency)?;
                self.remote = Some((remote, id));
                Ok(Some(raw_frame(FrameType::Auth, auth.encode().to_vec())?))
            }
            FrameType::Auth if self.remote.is_some() => {
                let remote = &self.remote.as_ref().ok_or(HandshakeError::Order)?.0;
                let transcript = self.transcript(remote);
                let auth = Auth::decode(frame.payload()).map_err(HandshakeError::Payload)?;
                let peer_role = match self.role {
                    ConnectionRole::Initiator => ConnectionRole::Acceptor,
                    ConnectionRole::Acceptor => ConnectionRole::Initiator,
                };
                transcript
                    .verify(peer_role, &remote.identity_public_key, &auth, crypto)
                    .map_err(HandshakeError::Dependency)?;
                let private = self.private.take().ok_or(HandshakeError::Order)?;
                self.keys = Some(
                    derive_keys(
                        self.role,
                        private,
                        &remote.ephemeral_public_key,
                        &transcript,
                        crypto,
                    )
                    .map_err(HandshakeError::Dependency)?,
                );
                self.peer_verified = true;
                Ok(None)
            }
            _ => Err(HandshakeError::Order),
        }
    }
    // Select role order explicitly so reversing HELLO arrival cannot change proof bytes.
    fn transcript(&self, remote: &Hello) -> Transcript {
        tracing::trace!(
            function = "Handshake::transcript",
            "Selecting HELLO transcript order"
        );
        match self.role {
            ConnectionRole::Initiator => Transcript::new(&self.local, remote),
            ConnectionRole::Acceptor => Transcript::new(remote, &self.local),
        }
    }
}

// Build bounded untagged HELLO/AUTH output without unchecked encoding assumptions.
fn raw_frame(kind: FrameType, payload: Vec<u8>) -> Result<WireFrame, HandshakeError> {
    tracing::trace!(
        function = "raw_frame",
        "Building an untagged handshake frame"
    );
    let header = FrameHeader::new(kind, 0, payload.len() as u32)
        .map_err(|e| HandshakeError::Framing(FramingError::Header(e)))?;
    WireFrame::new(header, payload, None).map_err(HandshakeError::Framing)
}

#[cfg(test)]
#[path = "state/tests.rs"]
mod tests;
