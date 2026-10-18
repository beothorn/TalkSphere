//! Stream identity and lifecycle rules, separate from wire framing and cryptography.
//! Connection orchestration owns this state and serializes mutations.
pub mod ids;
pub mod registry;
pub mod session;
