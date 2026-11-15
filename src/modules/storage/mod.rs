//! Paid immutable byte leases; Services callers supply authority and platform dependencies.
//!
//! Contract foundations only: no provider is registered or advertised until durable execution exists.
pub mod contract;
pub mod pricing;

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod fixture_tests;
