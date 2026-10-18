//! OS TCP and monotonic clock adapters; authentication remains an injected module capability.
mod clock;
mod reactor;
mod tcp;
pub use clock::SystemClock;
pub use tcp::TcpProvider;
#[cfg(test)]
mod tests;
