//! Replaceable platform and module dependencies; no adapters are implemented.

pub mod crypto;
pub mod identity;
pub mod io;

/// Composition supplies these capabilities, not values received from a peer.
/// Each Transport instance uses one purpose-scoped local Identity capability.
pub struct Dependencies<
    S: io::SocketProvider,
    T: io::Clock,
    I: identity::Identity,
    C: crypto::Cryptography,
> {
    pub sockets: S,
    pub clock: T,
    pub identity: I,
    pub cryptography: C,
}

/// Which boundary failed; separate from an explicit rejection by the peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dependency {
    Socket,
    Clock,
    Identity,
    Cryptography,
}

/// Secret-free machine-readable failure from an Identity/Cryptography adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyError {
    Unavailable,
    Denied,
    InvalidInput,
    VerificationFailed,
}
