//! Secret bearer claim portable format; never log or automatically display this type.
use super::Error;
use crate::modules::identity::api::IdentityId;
use zeroize::Zeroizing;
const PREFIX: &str = "cr2se-claim:";
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
/// Guarded bearer claim. Export must use a private operator-approved channel.
pub struct Portable {
    pub(super) issuer: IdentityId,
    pub(super) id: [u8; 32],
    pub(super) secret: Zeroizing<[u8; 32]>,
    pub(super) amount: u64,
}
impl Portable {
    /// Parse exactly one canonical 105-byte version-one claim without repairing aliases.
    /// The caller must also protect/erase its original input string.
    /// # Errors
    /// Invalid prefix/length/alphabet/version/amount is rejected without secret diagnostics.
    pub fn parse(text: &str) -> Result<Self, Error> {
        tracing::trace!("Parsing guarded portable credit claim");
        if text.len() != PREFIX.len() + 140 || !text.starts_with(PREFIX) {
            return Err(Error::Invalid);
        }
        let mut bytes = Zeroizing::new([0u8; 105]);
        for (i, group) in text.as_bytes()[PREFIX.len()..]
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
        {
            let mut n = 0u32;
            for b in group {
                let v = ALPHABET.iter().position(|a| a == b).ok_or(Error::Invalid)?;
                n = (n << 6) | v as u32;
            }
            bytes[i * 3..i * 3 + 3].copy_from_slice(&n.to_be_bytes()[1..]);
        }
        if bytes[0] != 1 {
            return Err(Error::Invalid);
        }
        let amount = u64::from_be_bytes(bytes[97..105].try_into().map_err(|_| Error::Invalid)?);
        if amount == 0 {
            return Err(Error::Invalid);
        }
        Ok(Self {
            issuer: IdentityId(bytes[1..33].try_into().map_err(|_| Error::Invalid)?),
            id: bytes[33..65].try_into().map_err(|_| Error::Invalid)?,
            secret: Zeroizing::new(bytes[65..97].try_into().map_err(|_| Error::Invalid)?),
            amount,
        })
    }
    /// Export secret bearer material into a guarded canonical string for explicit private delivery.
    pub fn export(&self) -> Zeroizing<String> {
        tracing::trace!("Exporting guarded claim through explicit private caller path");
        let mut bytes = Zeroizing::new([0u8; 105]);
        bytes[0] = 1;
        bytes[1..33].copy_from_slice(&self.issuer.0);
        bytes[33..65].copy_from_slice(&self.id);
        bytes[65..97].copy_from_slice(self.secret.as_ref());
        bytes[97..].copy_from_slice(&self.amount.to_be_bytes());
        let mut out = Zeroizing::new(String::from(PREFIX));
        for group in bytes.as_chunks::<3>().0 {
            let n = (u32::from(group[0]) << 16) | (u32::from(group[1]) << 8) | u32::from(group[2]);
            for shift in [18, 12, 6, 0] {
                out.push(ALPHABET[((n >> shift) & 63) as usize] as char);
            }
        }
        out
    }
    /// Return the public claim issuer without granting key or trust authority.
    pub fn issuer(&self) -> IdentityId {
        tracing::trace!("Reading public claim issuer");
        self.issuer
    }
    /// Return the public replay identity; knowledge alone cannot redeem a claim.
    pub fn id(&self) -> [u8; 32] {
        tracing::trace!("Reading claim replay identity");
        self.id
    }
    /// Return the portable hint; redemption always uses the issuer's stored amount.
    pub fn amount(&self) -> u64 {
        tracing::trace!("Reading portable claim amount hint");
        self.amount
    }
}

#[cfg(test)]
#[path = "tests/claim.rs"]
mod tests;
