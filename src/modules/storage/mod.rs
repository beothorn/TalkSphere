//! Paid immutable byte leases; Services callers supply authority and platform dependencies.
//!
//! Contract foundations only: no provider is registered or advertised until durable execution exists.
pub mod assessment;
pub mod clock_policy;
pub mod contract;
pub mod effect;
pub mod image;
pub mod journal;
pub mod lease;
pub mod operation;
pub mod pricing;
pub mod quota;
pub mod retention;
pub mod terms;

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod fixture_tests;

#[cfg(target_os = "linux")]
pub mod bytes;

#[cfg(target_os = "linux")]
pub mod recovery;
