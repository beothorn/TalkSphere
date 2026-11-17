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
#[cfg(target_os = "linux")]
mod mutation_result;
pub mod operation;
pub mod pricing;
pub mod quota;
#[cfg(target_os = "linux")]
pub mod removal;
pub mod renewal;
#[cfg(target_os = "linux")]
pub mod renewal_commit;
pub mod requester;
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

mod binding;

#[cfg(target_os = "linux")]
pub mod retrieve;

#[cfg(target_os = "linux")]
pub mod retrieve_result;
