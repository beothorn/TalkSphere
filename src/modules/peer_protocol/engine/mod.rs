//! Bounded execution primitives for authenticated Peer Protocol composition.
//! The Transport reactor owns scheduling; workers own synchronous dependency calls.
//! These primitives do not install handlers or register an application protocol.
pub mod worker;
