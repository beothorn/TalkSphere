//! Placement states, payload encodings and the fixed constants of Messaging version one.
//!
//! Callers: placement persistence, operations and signing. The state machine is
//! `receiving -> pending -> recovered | removed | expired`; `receiving` creates no obligation.

/// Service identifier of the standard offering.
pub const SERVICE: &str = "cr2se.messaging";
/// Service version.
pub const VERSION: u32 = 1;
/// Pricing model identifier.
pub const PRICING_MODEL: &str = "cr2se.messaging.v1";
/// Smallest encrypted payload: version byte, two keys, nonce and tag.
pub const MINIMUM_ENCRYPTED_BYTES: usize = 1 + 32 + 32 + 24 + 16;

/// Representation of a payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// No meaning is assigned to the bytes; they may be plaintext or someone else's ciphertext.
    Opaque,
    /// The interoperable CR2SE encrypted representation of section 6.
    IdentityEncryptedV1,
}

impl Encoding {
    /// The one-byte code used in the signing bytes.
    pub fn code(self) -> u8 {
        match self {
            Self::Opaque => 0x00,
            Self::IdentityEncryptedV1 => 0x01,
        }
    }

    /// The wire name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Opaque => "opaque",
            Self::IdentityEncryptedV1 => "cr2se.identity-encrypted.v1",
        }
    }

    /// Parse a wire name exactly.
    /// # Errors
    /// `None` for any other text (the caller reports an unsupported encoding).
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "opaque" => Some(Self::Opaque),
            "cr2se.identity-encrypted.v1" => Some(Self::IdentityEncryptedV1),
            _ => None,
        }
    }
}

/// Lifecycle state of a placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// An incomplete upload; no retention obligation.
    Receiving,
    /// Durably accepted; may be discovered and recovered.
    Pending,
    /// The first authorized recovery completed.
    Recovered,
    /// The sender ended it early.
    Removed,
    /// The retention ended while pending.
    Expired,
}

impl State {
    /// Whether moving to `next` is an allowed transition.
    pub fn may_become(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Receiving, Self::Pending)
                | (
                    Self::Pending,
                    Self::Recovered | Self::Removed | Self::Expired
                )
        )
    }

    /// Whether the provider still owes retention or recovery evidence in this state.
    pub fn is_active(self) -> bool {
        matches!(self, Self::Pending | Self::Recovered)
    }

    /// The status name reported to the sender, for the two states status may report.
    pub fn status_name(self) -> Option<&'static str> {
        match self {
            Self::Pending => Some("pending"),
            Self::Recovered => Some("recovered"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Codes and names are exactly the specified ones, and parsing accepts only those names.
    #[test]
    fn encodings_have_the_specified_codes_and_names() {
        assert_eq!(
            (Encoding::Opaque.code(), Encoding::Opaque.name()),
            (0, "opaque")
        );
        assert_eq!(
            (
                Encoding::IdentityEncryptedV1.code(),
                Encoding::IdentityEncryptedV1.name()
            ),
            (1, "cr2se.identity-encrypted.v1")
        );
        for e in [Encoding::Opaque, Encoding::IdentityEncryptedV1] {
            assert_eq!(Encoding::parse(e.name()), Some(e));
        }
        for bad in [
            "",
            "Opaque",
            "opaque ",
            "cr2se.identity-encrypted.v2",
            "plain",
        ] {
            assert_eq!(Encoding::parse(bad), None, "{bad:?}");
        }
        assert_eq!(MINIMUM_ENCRYPTED_BYTES, 105);
    }

    // Only the documented transitions are allowed; terminal states never move; only pending and recovered are active.
    #[test]
    fn the_state_machine_allows_only_its_transitions() {
        use State::*;
        let all = [Receiving, Pending, Recovered, Removed, Expired];
        let allowed = [
            (Receiving, Pending),
            (Pending, Recovered),
            (Pending, Removed),
            (Pending, Expired),
        ];
        for from in all {
            for to in all {
                assert_eq!(
                    from.may_become(to),
                    allowed.contains(&(from, to)),
                    "{from:?} -> {to:?}"
                );
            }
        }
        assert!(Pending.is_active() && Recovered.is_active());
        assert!(!Receiving.is_active() && !Removed.is_active() && !Expired.is_active());
        assert_eq!(Pending.status_name(), Some("pending"));
        assert_eq!(Removed.status_name(), None);
    }
}
