//! Protected platform storage adapters; algorithms and credential validation remain at explicit boundaries.
#[cfg(target_os = "linux")]
mod atomic;
#[cfg(target_os = "linux")]
mod file;
#[cfg(target_os = "linux")]
pub use file::FileStore;

#[cfg(not(target_os = "linux"))]
mod unsupported;
#[cfg(not(target_os = "linux"))]
pub use unsupported::FileStore;

#[cfg(all(test, target_os = "linux"))]
mod tests;

#[cfg(all(test, target_os = "linux"))]
#[path = "tests/contacts.rs"]
mod contact_tests;
#[cfg(target_os = "linux")]
mod contacts;

#[cfg(all(test, target_os = "linux"))]
#[path = "tests/recovery.rs"]
mod recovery_tests;
