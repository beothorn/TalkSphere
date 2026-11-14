//! Real strict-signature Ledger fixtures supply public owner ports, never constructed private transfer acceptances.
use super::world::ledger_context;
use crate::modules::{
    cryptography::{
        credential,
        error::CryptoError,
        ports::Entropy,
        primitives::{keys::signing_key, signature},
    },
    identity::api::IdentityId,
    ledger::{self, fees, transfer::Order},
    peer_protocol::authority::Authority,
};
use std::{collections::VecDeque, sync::Arc};
pub(in crate::modules::peer_protocol) struct Bytes(pub VecDeque<Vec<u8>>);
impl Entropy for Bytes {
    // Exact scripted entropy bounds are consumed once and fail closed on mismatch/exhaustion.
    fn fill(&mut self, output: &mut [u8]) -> Result<(), CryptoError> {
        let bytes = self.0.pop_front().ok_or(CryptoError::Unavailable)?;
        if bytes.len() != output.len() {
            return Err(CryptoError::Unavailable);
        }
        output.copy_from_slice(&bytes);
        Ok(())
    }
}
pub(in crate::modules::peer_protocol) struct Issuer {
    pub local: IdentityId,
    pub owner: IdentityId,
    pub recipient: IdentityId,
    pub peer: Arc<Authority>,
    pub authority: Arc<ledger::authority::Authority>,
    pub ledger: Arc<ledger::state::Ledger<ledger::persistence::Memory>>,
    pub quotes: fees::Quotes,
    pub order: Order,
    pub key: [u8; 32],
    pub signature: [u8; 64],
}
impl Issuer {
    // The original transfer owner derives from its real Ed25519 key; the issuer's Ledger starts with100 authorized credits.
    pub fn new() -> Self {
        let keys = signing_key(&mut Bytes(VecDeque::from([vec![1; 32]]))).unwrap();
        let owner = IdentityId(credential::identity_id(&keys.public_key).unwrap());
        let local = IdentityId([3; 32]);
        let recipient = IdentityId([2; 32]);
        let authority = Arc::new(ledger::authority::Authority::new());
        let peer = Arc::new(Authority::new());
        let admin = authority
            .grant(local, ledger::authority::Action::Administer, 16_777_216)
            .unwrap();
        let ledger = Arc::new(
            ledger::state::Ledger::initialize(
                local,
                authority.clone(),
                ledger::persistence::Memory::default(),
                Default::default(),
                &ledger_context(&admin),
                1000,
                100,
            )
            .unwrap(),
        );
        ledger.grant(&ledger_context(&admin), owner, 100).unwrap();
        let quotes = fees::Quotes::new(local, authority.clone(), 8).unwrap();
        let order = Order {
            issuer: local,
            owner,
            new_owner: recipient,
            transfer_id: [1; 16],
            amount: 9,
        };
        let signature = signature::sign(&keys.private_key, &order.encode().unwrap(), 121).unwrap();
        Self {
            local,
            owner,
            recipient,
            peer,
            authority,
            ledger,
            quotes,
            order,
            key: keys.public_key,
            signature,
        }
    }
    // Only real public quote/accept ports can create issuer commitment authority for these tests.
    pub fn commit(&self) -> ledger::transfer_state::TransferResult {
        let control = self
            .authority
            .grant(self.local, ledger::authority::Action::Control, 16_777_216)
            .unwrap();
        let c = ledger_context(&control);
        let peer = fees::Peer {
            owner: self.owner,
            connection: [8; 32],
            stream: 1,
        };
        let quote = self
            .quotes
            .quote(
                &c,
                peer,
                fees::Request {
                    order: self.order.clone(),
                    key: self.key,
                    signature: self.signature,
                    maximum_fee: 2,
                    fee: 2,
                    expires: 100,
                },
                0,
            )
            .unwrap();
        let accepted = self.quotes.accept(&c, &quote, peer, 2, 0).unwrap();
        self.ledger.commit_transfer(&c, &accepted).unwrap()
    }
}
