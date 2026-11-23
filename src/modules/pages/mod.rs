//! Static Pages: canonical paths, media types, page manifests and hosted page assembly (CR2SE `cr2se.page` and hosted pages).
//!
//! Callers: node composition and the hosted publication service. See [the module guide](mod.md).
pub mod error;
pub mod media;
pub mod path;

pub use error::Error;
