//! Exact-byte proof verification combines public identity validation with Cryptography; Transport owns freshness.
use super::{api::PublicIdentity, error::IdentityError, id, ports::Cryptography};

/// Borrowed untrusted public proof inputs; no private authority or session state is included.
pub struct Proof<'a> {
    /// Identity version; version one is supported.
    pub version: u8,
    /// Signing algorithm selector; Ed25519 is one.
    pub algorithm: u8,
    /// Claimed binary identity, exactly 32 bytes.
    pub claimed_id: &'a [u8],
    /// Canonical signing public key, exactly 32 bytes.
    pub public_key: &'a [u8],
    /// Exact bytes defined by the consuming protocol, including its role and freshness context.
    pub message: &'a [u8],
    /// Ordinary Ed25519 signature, exactly 64 bytes.
    pub signature: &'a [u8],
}

/// Validate the claimed identity and verify its exact supplied proof bytes within a local budget.
/// # Errors
/// Rejects oversized messages, malformed fields, unsupported selectors, ID mismatch, invalid proofs
/// and dependency failures; success does not create authentication, trust or replay state.
pub fn verify(
    crypto: &impl Cryptography,
    proof: Proof<'_>,
    maximum_bytes: usize,
) -> Result<PublicIdentity, IdentityError> {
    tracing::trace!("Verifying an identity proof over exact protocol-defined bytes");
    if proof.message.len() > maximum_bytes {
        return Err(IdentityError::ResourceLimit);
    }
    if proof.signature.len() != 64 {
        return Err(IdentityError::InvalidInput);
    }
    let identity = id::validate(
        crypto,
        proof.version,
        proof.algorithm,
        proof.public_key,
        proof.claimed_id,
    )?;
    crypto.verify(proof.public_key, proof.message, proof.signature)?;
    Ok(identity)
}

#[cfg(test)]
#[path = "tests/proof.rs"]
mod tests;
