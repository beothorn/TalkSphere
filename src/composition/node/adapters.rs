//! Native Linux Storage clock, entropy and hashing ports.
//!
//! Callers: the node owners and the examples. The clock pairs kernel uptime with UTC
//! and a boot identifier so retention can detect missing reboot evidence; across a
//! reboot no trusted elapsed time exists and Storage must refuse rather than guess.
use crate::modules::{
    cryptography::primitives::hash::sha256,
    services::Error,
    storage::contract::{Clock, Crypto, Time},
};
use std::time::{SystemTime, UNIX_EPOCH};

/// Source of raw platform readings, injectable for tests.
pub trait Platform: Send + Sync {
    /// Return the kernel boot identifier text.
    fn boot_id(&self) -> std::io::Result<String>;
    /// Return the kernel uptime text (`/proc/uptime` format).
    fn uptime(&self) -> std::io::Result<String>;
    /// Return UTC seconds since the Unix epoch, or `None` before the epoch.
    fn unix_seconds(&self) -> Option<u64>;
}

/// The real Linux platform readings.
pub struct Linux;
impl Platform for Linux {
    // Read the per-boot random identifier.
    fn boot_id(&self) -> std::io::Result<String> {
        tracing::trace!("Reading the kernel boot identifier");
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
    }
    // Read kernel uptime, which keeps counting across process restarts within one boot.
    fn uptime(&self) -> std::io::Result<String> {
        tracing::trace!("Reading kernel uptime");
        std::fs::read_to_string("/proc/uptime")
    }
    // Read wall-clock UTC.
    fn unix_seconds(&self) -> Option<u64> {
        tracing::trace!("Reading UTC");
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs())
    }
}

/// Storage clock combining uptime, UTC and a hashed boot identifier.
pub struct NativeClock {
    platform: Box<dyn Platform>,
    boot: [u8; 16],
}
impl NativeClock {
    /// Build the clock on the real platform.
    /// # Errors
    /// The boot identifier is unreadable.
    pub fn new() -> Result<Self, String> {
        tracing::trace!("Preparing the native Storage clock");
        Self::with_platform(Box::new(Linux))
    }

    /// Build the clock over supplied platform readings.
    /// # Errors
    /// The boot identifier is unreadable or the hash fails.
    pub fn with_platform(platform: Box<dyn Platform>) -> Result<Self, String> {
        tracing::trace!("Preparing a Storage clock over supplied platform readings");
        let id = platform
            .boot_id()
            .map_err(|_| "boot identifier is unavailable".to_string())?;
        let digest = sha256(id.as_bytes(), id.len().max(1))
            .map_err(|_| "boot identifier could not be hashed".to_string())?;
        let mut boot = [0; 16];
        boot.copy_from_slice(&digest[..16]);
        Ok(Self { platform, boot })
    }
}
impl Clock for NativeClock {
    // Sample uptime and UTC together; uptime is comparable across restarts within one boot.
    fn now(&self) -> Result<Time, Error> {
        tracing::trace!("Sampling native uptime and UTC for Storage retention");
        let uptime = self.platform.uptime().map_err(|_| Error::Unavailable)?;
        let elapsed_seconds = uptime
            .split_whitespace()
            .next()
            .and_then(|s| s.split('.').next())
            .and_then(|s| s.parse().ok())
            .ok_or(Error::Invalid)?;
        let unix_seconds = self.platform.unix_seconds().ok_or(Error::Invalid)?;
        Ok(Time {
            unix_seconds,
            elapsed_seconds,
            boot: self.boot,
        })
    }
}

/// Operating-system entropy for opaque lease identifiers and SHA-256 hashing.
pub struct SystemCrypto;
impl Crypto for SystemCrypto {
    // Obtain an unpredictable identifier from host entropy; failure is explicit.
    fn lease_id(&self) -> Result<[u8; 32], Error> {
        tracing::trace!("Generating an opaque Storage lease identifier");
        let mut id = [0; 32];
        getrandom::fill(&mut id).map_err(|_| Error::Unavailable)?;
        Ok(id)
    }

    // Hash exact logical bytes; oversize input is a limit failure.
    fn hash(&self, bytes: &[u8]) -> Result<[u8; 32], Error> {
        tracing::trace!("Hashing exact Storage object bytes");
        sha256(bytes, bytes.len()).map_err(|_| Error::Limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixed {
        boot: Option<&'static str>,
        uptime: &'static str,
        unix: Option<u64>,
    }
    impl Platform for Fixed {
        fn boot_id(&self) -> std::io::Result<String> {
            self.boot
                .map(str::to_string)
                .ok_or_else(|| std::io::Error::other("missing"))
        }
        fn uptime(&self) -> std::io::Result<String> {
            Ok(self.uptime.to_string())
        }
        fn unix_seconds(&self) -> Option<u64> {
            self.unix
        }
    }
    // Readings are combined exactly, and a different boot identifier yields a different boot tag.
    #[test]
    fn clock_combines_readings_and_distinguishes_boots() {
        let make = |boot| {
            NativeClock::with_platform(Box::new(Fixed {
                boot: Some(boot),
                uptime: "123.45 9.0\n",
                unix: Some(1_000),
            }))
            .unwrap()
        };
        let a = make("boot-a").now().unwrap();
        assert_eq!((a.unix_seconds, a.elapsed_seconds), (1_000, 123));
        assert_ne!(a.boot, make("boot-b").now().unwrap().boot);
        assert_eq!(a.boot, make("boot-a").now().unwrap().boot);
    }
    // Unreadable boot evidence, malformed uptime and a pre-epoch clock fail explicitly.
    #[test]
    fn clock_failures_are_explicit() {
        assert!(
            NativeClock::with_platform(Box::new(Fixed {
                boot: None,
                uptime: "1",
                unix: Some(1)
            }))
            .is_err()
        );
        let bad = |uptime, unix| {
            NativeClock::with_platform(Box::new(Fixed {
                boot: Some("b"),
                uptime,
                unix,
            }))
            .unwrap()
            .now()
        };
        assert!(bad("nonsense", Some(1)).is_err());
        assert!(bad("1.0", None).is_err());
    }
    // Real hashing is exact and identifiers differ between calls.
    #[test]
    fn system_crypto_hashes_and_generates_distinct_ids() {
        let crypto = SystemCrypto;
        let hash = crypto.hash(b"abc").unwrap();
        assert_eq!(
            hash[..4],
            [0xba, 0x78, 0x16, 0xbf],
            "SHA-256 of abc starts ba7816bf"
        );
        assert_ne!(crypto.lease_id().unwrap(), crypto.lease_id().unwrap());
    }
    // The real Linux platform provides a usable clock sample.
    #[test]
    fn linux_platform_samples() {
        let now = NativeClock::new().unwrap().now().unwrap();
        assert!(now.unix_seconds > 1_600_000_000);
    }
}
