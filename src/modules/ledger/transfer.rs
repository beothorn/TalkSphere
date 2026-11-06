//! Canonical version-one unsigned transfer orders, independent of JSON control binding.
use super::Error;
use crate::modules::identity::api::IdentityId;
/// Exact owner-authorized transfer fields; public key/signature remain external.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    /// Fresh cryptographically random 16-byte replay identity.
    pub transfer_id: [u8; 16],
    /// Identity whose local Ledger recognizes the credits.
    pub issuer: IdentityId,
    /// Current owner who must authenticate and sign.
    pub owner: IdentityId,
    /// New owner, distinct from owner and issuer.
    pub new_owner: IdentityId,
    /// Positive gross amount including issuer fee.
    pub amount: u64,
}
impl Order {
    /// Encode exactly the 121 ordinary-Ed25519 signing bytes.
    /// # Errors
    /// Invalid zero amount or forbidden recipient fails before encoding.
    pub fn encode(&self) -> Result<[u8; 121], Error> {
        tracing::trace!("Encoding canonical Ledger transfer order");
        if self.amount == 0 || self.new_owner == self.owner || self.new_owner == self.issuer {
            return Err(Error::Invalid);
        }
        let mut out = [0; 121];
        out[0] = 1;
        out[1..17].copy_from_slice(&self.transfer_id);
        out[17..49].copy_from_slice(&self.issuer.0);
        out[49..81].copy_from_slice(&self.owner.0);
        out[81..113].copy_from_slice(&self.new_owner.0);
        out[113..121].copy_from_slice(&self.amount.to_be_bytes());
        Ok(out)
    }
    /// Decode only the canonical 121-byte version-one layout with no trailing bytes.
    /// # Errors
    /// Invalid version/length/amount/recipient fails without semantic repair.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        tracing::trace!("Decoding canonical Ledger transfer order");
        if bytes.len() != 121 || bytes[0] != 1 {
            return Err(Error::Invalid);
        }
        let order = Self {
            transfer_id: bytes[1..17].try_into().map_err(|_| Error::Invalid)?,
            issuer: IdentityId(bytes[17..49].try_into().map_err(|_| Error::Invalid)?),
            owner: IdentityId(bytes[49..81].try_into().map_err(|_| Error::Invalid)?),
            new_owner: IdentityId(bytes[81..113].try_into().map_err(|_| Error::Invalid)?),
            amount: u64::from_be_bytes(bytes[113..121].try_into().map_err(|_| Error::Invalid)?),
        };
        order.encode()?;
        Ok(order)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Independently specified byte regions pin every canonical offset and reject the old layout.
    #[test]
    fn literal_order_and_refusals() {
        let order = Order {
            transfer_id: [7; 16],
            issuer: IdentityId([3; 32]),
            owner: IdentityId([1; 32]),
            new_owner: IdentityId([2; 32]),
            amount: 9,
        };
        let mut literal = vec![1];
        literal.extend([7; 16]);
        literal.extend([3; 32]);
        literal.extend([1; 32]);
        literal.extend([2; 32]);
        literal.extend([0, 0, 0, 0, 0, 0, 0, 9]);
        assert_eq!(order.encode().unwrap().as_slice(), literal);
        assert_eq!(Order::decode(&literal), Ok(order.clone()));
        assert_eq!(&literal[113..], &[0, 0, 0, 0, 0, 0, 0, 9]);
        assert!(Order::decode(&literal[..105]).is_err());
        for i in 0..121 {
            assert!(Order::decode(&literal[..i]).is_err())
        }
        let mut bad = literal.clone();
        bad.push(0);
        assert!(Order::decode(&bad).is_err());
        bad = literal.clone();
        bad[0] = 2;
        assert!(Order::decode(&bad).is_err());
        assert!(
            Order {
                amount: 0,
                ..order.clone()
            }
            .encode()
            .is_err()
        );
        assert!(
            Order {
                new_owner: order.owner,
                ..order.clone()
            }
            .encode()
            .is_err()
        );
        assert!(
            Order {
                new_owner: order.issuer,
                ..order
            }
            .encode()
            .is_err()
        );
    }
}
