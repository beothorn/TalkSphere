//! Fixed-length identifiers and hashes of the Hosted Publications service.
//!
//! Callers: every other Hosted Publications file. A publication ID is chosen by the publisher; placement and removal IDs reuse
//! the Messaging identifier types, whose meaning is identical. Values never print their bytes.
use super::error::Error;

pub use crate::modules::messaging::ids::{PlacementId, RemovalId, SIGNATURE_BYTES, Signature};

/// Length of every identifier, hash and public key.
pub const ID_BYTES: usize = 32;

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
                tracing::trace!("Decoding a fixed-length hosted publication identifier");
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
    /// Publisher-chosen identifier of one publication, shared by every placement of it.
    PublicationId
);
fixed_id!(
    /// SHA-256 of a publication's content.
    ContentHash
);

#[cfg(test)]
mod tests {
    use super::*;

    // Exactly 32 bytes decode, other lengths are refused, values never print their bytes and identifiers order by bytes.
    #[test]
    fn lengths_are_exact_ordered_and_not_printed() {
        assert!(PublicationId::from_slice(&[7; 32]).is_ok());
        for n in [0, 31, 33, 64] {
            assert_eq!(
                ContentHash::from_slice(&vec![0; n]),
                Err(Error::Invalid),
                "{n}"
            );
        }
        assert!(!format!("{:?}", PublicationId([9; 32])).contains('9'));
        assert!(PublicationId([1; 32]) < PublicationId([2; 32]));
    }
}
