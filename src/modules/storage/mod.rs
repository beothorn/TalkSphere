//! Paid immutable byte leases; Services callers supply authority and platform dependencies.
//!
//! Contract foundations only: no provider is registered or advertised until durable execution exists.
pub mod contract;

#[cfg(test)]
mod test_support;
