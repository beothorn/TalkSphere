//! Directional session material derived through opaque Crypto capabilities.
//! Consumed secrets are dropped on every failure path; no secret bytes are logged.
use super::Transcript;
use crate::modules::transport::{
    api::ConnectionRole,
    ports::{DependencyError, crypto::Cryptography},
};

/// One direction's opaque integrity key and connection-specific nonce prefix.
pub struct DirectionKeys<K> {
    /// Adapter-owned key; Drop must release/erase its secret material.
    pub key: K,
    /// Sixteen bytes preceding the direction's implicit sequence counter.
    pub nonce_prefix: [u8; 16],
}
/// Locally oriented keys; initiator and acceptor have opposite assignments.
pub struct SessionKeys<K> {
    /// Material used for local outbound frames.
    pub send: DirectionKeys<K>,
    /// Material used for remote inbound frames.
    pub receive: DirectionKeys<K>,
}

/// Request the exact X25519/HKDF construction and orient its results by TCP role.
/// # Errors
/// Propagates every Crypto failure; agree must reject the all-zero shared secret.
/// Ownership of the ephemeral secret is consumed even if a later operation fails.
pub fn derive_keys<C: Cryptography>(
    role: ConnectionRole,
    private: C::EphemeralSecret,
    remote: &[u8; 32],
    transcript: &Transcript,
    crypto: &mut C,
) -> Result<SessionKeys<C::IntegrityKey>, DependencyError> {
    tracing::trace!(
        function = "derive_keys",
        "Deriving connection-bound directional material"
    );
    let shared = crypto.agree(private, remote)?;
    let salt_input = [
        b"CR2SE-NETWORK-KEYS-V1".as_slice(),
        &[0],
        transcript.as_bytes().as_slice(),
    ]
    .concat();
    let salt = crypto.sha256(&salt_input)?;
    let prk = crypto.extract(&salt, shared)?;
    let i2a_key = crypto.expand_key(&prk, b"CR2SE-NETWORK-V1-I2A-KEY")?;
    let a2i_key = crypto.expand_key(&prk, b"CR2SE-NETWORK-V1-A2I-KEY")?;
    let i2a_prefix = crypto.expand_nonce_prefix(&prk, b"CR2SE-NETWORK-V1-I2A-NONCE")?;
    let a2i_prefix = crypto.expand_nonce_prefix(&prk, b"CR2SE-NETWORK-V1-A2I-NONCE")?;
    let i2a = DirectionKeys {
        key: i2a_key,
        nonce_prefix: i2a_prefix,
    };
    let a2i = DirectionKeys {
        key: a2i_key,
        nonce_prefix: a2i_prefix,
    };
    Ok(match role {
        ConnectionRole::Initiator => SessionKeys {
            send: i2a,
            receive: a2i,
        },
        ConnectionRole::Acceptor => SessionKeys {
            send: a2i,
            receive: i2a,
        },
    })
}

/// Combine a direction-specific prefix and big-endian implicit sequence number.
pub fn nonce(prefix: &[u8; 16], sequence: u64) -> [u8; 24] {
    tracing::trace!(
        function = "nonce",
        "Constructing the direction sequence nonce"
    );
    let mut result = [0; 24];
    result[..16].copy_from_slice(prefix);
    result[16..].copy_from_slice(&sequence.to_be_bytes());
    result
}
