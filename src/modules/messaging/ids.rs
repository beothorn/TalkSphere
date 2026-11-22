//! Fixed-length opaque identifiers and byte values of the Messaging service.
//!
//! Callers: every other Messaging file. Each identifier is exactly 32 random bytes chosen by its
//! owner: the sender chooses message and removal IDs, the recipient recovery IDs and the provider
//! placement IDs. None is a key, password or bearer token; knowing one authorizes nothing. Values
//! never print their bytes, so identifiers cannot leak through diagnostics.
use super::error::Error;

/// Length of every identifier, hash and public key.
pub const ID_BYTES: usize = 32;
/// Length of an Ed25519 signature.
pub const SIGNATURE_BYTES: usize = 64;

macro_rules! fixed_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub [u8; ID_BYTES]);
        impl $name {
            /// Decode exactly 32 bytes.
            /// # Errors
            /// `Invalid` for any other length.
            pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
                tracing::trace!("Decoding a fixed-length Messaging identifier");
                bytes.try_into().map(Self).map_err(|_| Error::Invalid)
            }
        }
        impl std::fmt::Debug for $name {
            // Identifiers are never printed.
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, concat!(stringify!($name), "(..)"))
            }
        }
    };
}

fixed_id!(
    /// Sender-chosen identifier of one signed message, shared by every placement of it.
    MessageId
);
fixed_id!(
    /// Provider-chosen identifier of one placement; unique at that provider.
    PlacementId
);
fixed_id!(
    /// Recipient-chosen replay identifier of one recovery.
    RecoveryId
);
fixed_id!(
    /// Sender-chosen replay identifier of one removal.
    RemovalId
);
fixed_id!(
    /// SHA-256 of a payload.
    PayloadHash
);

/// A 64-byte Ed25519 signature.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Signature(pub [u8; SIGNATURE_BYTES]);
impl Signature {
    /// Decode exactly 64 bytes.
    /// # Errors
    /// `Invalid` for any other length.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        tracing::trace!("Decoding a Messaging signature");
        bytes.try_into().map(Self).map_err(|_| Error::Invalid)
    }
}
impl std::fmt::Debug for Signature {
    // Signatures are public but still never printed, to keep logs free of protocol values.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Signature(..)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Exactly 32 (or 64) bytes decode; every other length is refused; values never print their bytes.
    #[test]
    fn lengths_are_exact_and_values_are_not_printed() {
        assert!(MessageId::from_slice(&[7; 32]).is_ok());
        for n in [0, 1, 31, 33, 64] {
            assert_eq!(
                MessageId::from_slice(&vec![0; n]),
                Err(Error::Invalid),
                "{n}"
            );
        }
        assert!(Signature::from_slice(&[1; 64]).is_ok());
        assert_eq!(Signature::from_slice(&[1; 63]), Err(Error::Invalid));
        assert_eq!(Signature::from_slice(&[1; 65]), Err(Error::Invalid));
        assert!(!format!("{:?}", PlacementId([9; 32])).contains('9'));
        assert!(!format!("{:?}", Signature([9; 64])).contains('9'));
    }

    // Identifier kinds are distinct types that order by their bytes, so cursors sort lexicographically.
    #[test]
    fn identifiers_order_by_bytes() {
        assert!(PlacementId([1; 32]) < PlacementId([2; 32]));
        let mut low = [0u8; 32];
        low[31] = 1;
        assert!(PlacementId(low) < PlacementId([1; 32]));
    }
}
