//! Paid immutable byte leases; Services callers supply authority and platform dependencies.
//!
//! Contract foundations only: no provider is registered or advertised until durable execution exists.
pub mod clock_policy;
pub mod contract;
pub mod lease;
pub mod operation;
pub mod pricing;
pub mod retention;
pub mod terms;

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod fixture_tests;
