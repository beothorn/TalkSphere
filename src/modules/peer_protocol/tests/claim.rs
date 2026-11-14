//! Claim controls use real authenticated generic encryption and owner-specific Ledger replay without clear bearer data or administrator wire authority.
use super::{
    Error,
    authority::{Action, Authority},
    contract::{Association, Limits, OperationKey},
    exchange::{Direction, Exchange, Outcome},
    ledger_control::claim::{self, Ports, Request},
    message::{Kind, Message},
    record::Record,
    test_support::{
        ledger::Bytes,
        world::{context, ledger_context},
    },
};
use crate::modules::{
    cryptography::{
        api::{DecryptRequest, IdentityEncryption, KeyAgreement, RecipientCredential, Signatures},
        config::ResourceLimits,
        credential,
        provider::Provider,
        random::SystemEntropy,
        secrets::{Persistent, Secret},
    },
    identity::api::IdentityId,
    ledger::{
        self,
        redemption::{Decryptor, ENVELOPE_BYTES},
    },
};
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use zeroize::Zeroizing;
struct Opener {
    crypto: Provider<SystemEntropy>,
    key: Secret<Persistent>,
    id: [u8; 32],
    public: [u8; 32],
    calls: AtomicUsize,
}
impl Decryptor for Opener {
    // The real generic provider authenticates its entire ciphertext before returning guarded plaintext; no test stub approves bytes.
    fn decrypt(&self, object: &[u8]) -> Result<Zeroizing<Vec<u8>>, ledger::Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.crypto
            .decrypt(DecryptRequest {
                object,
                identity_id: self.id,
                encryption_public_key: self.public,
                private_key: &self.key,
                limits: ResourceLimits::new(4096, 4096).unwrap(),
            })
            .map(Zeroizing::new)
            .map_err(|_| ledger::Error::Denied)
    }
}
// Genuine issuer recipient credentials bind independent Ed25519 and X25519 keys for encrypted redemption.
fn opener() -> (Opener, RecipientCredential) {
    let mut crypto = Provider::system(4096).unwrap();
    let signing = crypto.generate_signing_key().unwrap();
    let encryption = crypto.generate_persistent_key().unwrap();
    let id = credential::identity_id(&signing.public_key).unwrap();
    let recipient = RecipientCredential {
        identity_id: id,
        signing_public_key: signing.public_key,
        encryption_public_key: encryption.public_key,
        binding_signature: credential::bind(&signing.private_key, &encryption.public_key).unwrap(),
    };
    (
        Opener {
            crypto,
            key: encryption.private_key,
            id,
            public: encryption.public_key,
            calls: AtomicUsize::new(0),
        },
        recipient,
    )
}
// Original owner replays return the same stored amount; tampering, wrong owner and malformed/oversized envelopes never issue credits.
#[test]
fn encrypted_claim_wire_redemption_and_owner_replay() {
    let (mut opener, recipient) = opener();
    let issuer = IdentityId(opener.id);
    let owner = IdentityId([2; 32]);
    let authority = Arc::new(ledger::authority::Authority::new());
    let admin = authority
        .grant(issuer, ledger::authority::Action::Administer, 16_777_216)
        .unwrap();
    let control = authority
        .grant(issuer, ledger::authority::Action::Control, 16_777_216)
        .unwrap();
    let lc = ledger_context(&control);
    let ledger = Arc::new(
        ledger::state::Ledger::initialize(
            issuer,
            authority.clone(),
            ledger::persistence::Memory::default(),
            Default::default(),
            &ledger_context(&admin),
            1000,
            100,
        )
        .unwrap(),
    );
    let portable = ledger
        .create_claim(
            &ledger_context(&admin),
            [1; 32],
            7,
            Some(owner),
            &mut Bytes(VecDeque::from([vec![4; 32], vec![5; 32]])),
        )
        .unwrap();
    let object = portable
        .encrypt(owner, &recipient, &mut opener.crypto)
        .unwrap();
    let opener = Arc::new(opener);
    assert_eq!(object.len(), ENVELOPE_BYTES);
    assert!(!object.windows(32).any(|b| b == [5; 32]));
    let peer = Arc::new(Authority::new());
    let pg = peer.grant(issuer, Action::Invoke, 16_777_216).unwrap();
    let pc = context(&pg);
    let caller = Arc::new(Authority::new());
    let rg = caller.grant(owner, Action::Request, 16_777_216).unwrap();
    let rc = context(&rg);
    let ig = caller.grant(owner, Action::Invoke, 16_777_216).unwrap();
    let ic = context(&ig);
    for stream in [1, 3] {
        let operation = OperationKey {
            requester: owner,
            connection: [8; 32],
            stream,
        };
        let outgoing = Association {
            local: owner,
            peer: issuer,
            operation,
        };
        let incoming = Association {
            local: issuer,
            peer: owner,
            operation,
        };
        let request = Request::new(&rc, caller.clone(), outgoing, &object, portable.id()).unwrap();
        let mut wire = Message::parse(request.bytes(), 65536)
            .unwrap()
            .value()
            .clone();
        wire["amount"] = serde_json::json!("9999");
        let adapter = crate::composition::peer::claims::Claims::new(
            ledger.clone(),
            authority.clone(),
            peer.clone(),
            opener.clone(),
            Limits::default(),
        )
        .unwrap();
        let result = crate::modules::peer_protocol::engine::endpoint::Claims::redeem(
            &adapter,
            incoming,
            pc.deadline,
            &Message::parse(wire.to_string().as_bytes(), 65536).unwrap(),
        )
        .unwrap();
        let Record::Control(bytes) = result else {
            panic!("expected public claim result")
        };
        let result = Message::parse(&bytes, 65536).unwrap();
        assert_eq!(result.value()["amount"], "7");
        let completed = request.finish(&ic, outgoing, &result, true).unwrap();
        assert_eq!(completed.issuer, issuer);
        assert_eq!(completed.id, portable.id());
        assert_eq!(completed.amount, 7);
    }
    let read = authority
        .grant(issuer, ledger::authority::Action::Read, 16_777_216)
        .unwrap();
    assert_eq!(
        ledger.view(&ledger_context(&read), owner).unwrap().issued,
        7
    );
    for case in 0..4 {
        let mut bad = object.clone();
        let mut authenticated = owner;
        match case {
            0 => {
                bad.pop();
            }
            1 => bad.push(0),
            2 => *bad.last_mut().unwrap() ^= 1,
            _ => authenticated = IdentityId([3; 32]),
        }
        let a = Association {
            local: issuer,
            peer: authenticated,
            operation: OperationKey {
                requester: authenticated,
                connection: [9; 32],
                stream: 5,
            },
        };
        let message=Message::parse(serde_json::json!({"type":"ledger.claim.redeem","envelope":super::base64::encode(&bad)}).to_string().as_bytes(),65536).unwrap();
        let before = opener.calls.load(Ordering::SeqCst);
        assert!(
            claim::respond(
                &pc,
                peer.as_ref(),
                a,
                &message,
                Ports {
                    owner: &ledger,
                    control: &lc,
                    decryptor: opener.as_ref()
                },
                Limits::default()
            )
            .is_err()
        );
        if case < 2 {
            assert_eq!(opener.calls.load(Ordering::SeqCst), before);
        }
    }
    assert_eq!(
        ledger.view(&ledger_context(&read), owner).unwrap().issued,
        7
    );
    let mut exchange = Exchange::new(Direction::Local);
    exchange
        .control(Direction::Local, Kind::LedgerClaimRedeem)
        .unwrap();
    assert_eq!(exchange.close(false), Ok(Outcome::Unknown));
}
struct Unavailable(AtomicUsize);
impl Decryptor for Unavailable {
    // A missing protected key returns a typed dependency error and no plaintext.
    fn decrypt(&self, _: &[u8]) -> Result<Zeroizing<Vec<u8>>, ledger::Error> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(ledger::Error::Unavailable)
    }
}
// Missing decryption cannot issue credits, and incomplete/wrong-ID issuer replies cannot become a successful requester observation.
#[test]
fn claim_dependency_and_completion_failures_are_explicit() {
    let f = super::test_support::ledger::Issuer::new();
    let owner = f.owner;
    let a = Association {
        local: owner,
        peer: f.local,
        operation: OperationKey {
            requester: owner,
            connection: [8; 32],
            stream: 1,
        },
    };
    let auth = Arc::new(Authority::new());
    let rg = auth.grant(owner, Action::Request, 16_777_216).unwrap();
    let rc = context(&rg);
    let ig = auth.grant(owner, Action::Invoke, 16_777_216).unwrap();
    let ic = context(&ig);
    for case in 0..3 {
        let request =
            Request::new(&rc, auth.clone(), a, &vec![0; ENVELOPE_BYTES], [1; 32]).unwrap();
        let response=Message::parse(serde_json::json!({"type":"ledger.claim.result","claimId":super::base64::encode(&[if case==1 {2}else{1};32]),"amount":if case==2 {"0"}else{"7"}}).to_string().as_bytes(),65536).unwrap();
        assert!(request.finish(&ic, a, &response, case != 0).is_err());
    }
    let request = Request::new(&rc, auth.clone(), a, &vec![0; ENVELOPE_BYTES], [1; 32]).unwrap();
    let message = Message::parse(request.bytes(), 65536).unwrap();
    let incoming = Association {
        local: f.local,
        peer: owner,
        ..a
    };
    let pg = f.peer.grant(f.local, Action::Invoke, 16_777_216).unwrap();
    let lg = f
        .authority
        .grant(f.local, ledger::authority::Action::Control, 16_777_216)
        .unwrap();
    let lc = ledger_context(&lg);
    let decryptor = Unavailable(AtomicUsize::new(0));
    assert_eq!(
        claim::respond(
            &context(&pg),
            f.peer.as_ref(),
            incoming,
            &message,
            Ports {
                owner: f.ledger.as_ref(),
                control: &lc,
                decryptor: &decryptor
            },
            Limits::default()
        ),
        Err(Error::Unavailable)
    );
    assert_eq!(decryptor.0.load(Ordering::SeqCst), 1);
    // A locally unrepresentable response fails before decryption or issuance.
    let limits = Limits {
        nodes: 3,
        ..Default::default()
    };
    assert_eq!(
        claim::respond(
            &context(&pg),
            f.peer.as_ref(),
            incoming,
            &message,
            Ports {
                owner: f.ledger.as_ref(),
                control: &lc,
                decryptor: &decryptor
            },
            limits
        ),
        Err(Error::Limit)
    );
    assert_eq!(decryptor.0.load(Ordering::SeqCst), 1);
    assert_eq!(f.ledger.balance(&lc, owner), Ok(100));
}
