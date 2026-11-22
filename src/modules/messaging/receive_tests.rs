//! Tests for authenticating, deduplicating and decrypting recovered messages (MS-17 and the validation failures).
use super::{encryption::x25519_public, *};
use crate::modules::messaging::{
    ids::{PlacementId, RecoveryId},
    metadata::{Metadata, SignedMessage},
    operation::Receipt,
    ports::Signer,
    signing::sign_message,
    test_support::{SequenceEntropy, TestIdentity},
};

const PLACEMENT: PlacementId = PlacementId([7; 32]);

// A named edit applied to a recovery result to damage it.
type Damage = (&'static str, Box<dyn Fn(&mut RecoverOutput)>);

// Everything the tests need: a sender (seed 1), provider (seed 2), recipient (seed 3) and the recipient's X25519 key pair.
struct World {
    sender: TestIdentity,
    provider: TestIdentity,
    recipient: TestIdentity,
    secret: Secret<Persistent>,
    public: [u8; 32],
}

fn world() -> World {
    let seed = [0x60u8; 32];
    World {
        sender: TestIdentity::new(1),
        provider: TestIdentity::new(2),
        recipient: TestIdentity::new(3),
        secret: Secret::new(Zeroizing::new(seed)),
        public: x25519_public(&seed),
    }
}

fn recipient(w: &World) -> Recipient<'_> {
    Recipient {
        id: w.recipient.identity(),
        encryption_key: &w.secret,
        encryption_public_key: w.public,
    }
}

fn provider_key(w: &World) -> ProviderKey {
    ProviderKey {
        id: w.provider.identity(),
        public_key: w.provider.public_key(),
    }
}

// A signed, optionally encrypted message with the given ID from the sender to the recipient.
fn message(w: &World, id: u8, plaintext: &[u8], encrypted: bool) -> SignedMessage {
    let message_id = MessageId([id; 32]);
    let payload = if encrypted {
        let metadata = Metadata {
            message_id,
            sender_id: w.sender.identity(),
            recipient_id: w.recipient.identity(),
            sender_created_at: 1000,
            encoding: Encoding::IdentityEncryptedV1,
            payload_size: 0,
            payload_hash: PayloadHash([0; 32]),
        };
        encryption::encrypt(
            plaintext,
            &metadata,
            &w.public,
            &SequenceEntropy::new(u64::from(id)),
        )
        .unwrap()
    } else {
        plaintext.to_vec()
    };
    let encoding = if encrypted {
        Encoding::IdentityEncryptedV1
    } else {
        Encoding::Opaque
    };
    sign_message(
        &w.sender,
        w.recipient.identity(),
        message_id,
        1000,
        encoding,
        payload,
    )
    .unwrap()
}

// What a provider returns for a recovery of `message` under `placement`, with both signatures made by the real keys.
fn recovered(w: &World, placement: PlacementId, message: SignedMessage) -> RecoverOutput {
    let m = &message.metadata;
    let recovery_id = RecoveryId([5; 32]);
    let request = receipt::sign_request(
        &w.recipient,
        w.provider.identity(),
        placement,
        m.message_id,
        m.payload_hash,
        recovery_id,
    )
    .unwrap();
    let binding = Binding {
        provider_id: w.provider.identity(),
        placement_id: placement,
        message_id: m.message_id,
        sender_id: m.sender_id,
        recipient_id: m.recipient_id,
        payload_hash: m.payload_hash,
    };
    let provider_signature =
        receipt::sign_receipt(&w.provider, &binding, recovery_id, 1005).unwrap();
    let receipt = Receipt {
        recovery_id,
        recovered_at: 1005,
        recipient_public_key: request.recipient_public_key,
        recipient_signature: request.recipient_signature,
        provider_signature,
    };
    RecoverOutput {
        placement_id: placement,
        message,
        receipt,
    }
}

// An encrypted message is authenticated and then decrypted to the exact plaintext; an opaque message is delivered as received.
#[test]
fn a_valid_message_is_authenticated_then_decrypted() {
    let w = world();
    let inbox = Inbox::new(8);
    let out = recovered(&w, PLACEMENT, message(&w, 1, b"hello recipient", true));
    let Arrival::New(got) = inbox
        .accept(&recipient(&w), &provider_key(&w), &out)
        .unwrap()
    else {
        panic!("new")
    };
    assert_eq!(
        (got.content.as_slice(), got.sender_id, got.encoding),
        (
            &b"hello recipient"[..],
            w.sender.identity(),
            Encoding::IdentityEncryptedV1
        )
    );
    let plain = recovered(&w, PLACEMENT, message(&w, 2, b"opaque bytes", false));
    let Arrival::New(got) = inbox
        .accept(&recipient(&w), &provider_key(&w), &plain)
        .unwrap()
    else {
        panic!("new")
    };
    assert_eq!(&*got.content, b"opaque bytes");
    assert_eq!(inbox.len(), 2);
    assert!(
        !format!("{got:?}").contains("opaque"),
        "content is never printed"
    );
}

// MS-17: the same message recovered from several providers (different placements and receipts) is delivered once; a different message
// from the same sender is new; and the same ID with a different signed payload is a conflict that delivers nothing.
#[test]
fn replicas_of_one_message_are_delivered_once() {
    let w = world();
    let inbox = Inbox::new(8);
    let original = message(&w, 1, b"same message", true);
    let at_first = recovered(&w, PlacementId([7; 32]), original.clone());
    let at_second = recovered(&w, PlacementId([8; 32]), original);
    assert!(matches!(
        inbox.accept(&recipient(&w), &provider_key(&w), &at_first),
        Ok(Arrival::New(_))
    ));
    assert!(matches!(
        inbox.accept(&recipient(&w), &provider_key(&w), &at_second),
        Ok(Arrival::Duplicate)
    ));
    let other = recovered(&w, PLACEMENT, message(&w, 2, b"another", true));
    assert!(matches!(
        inbox.accept(&recipient(&w), &provider_key(&w), &other),
        Ok(Arrival::New(_))
    ));
    let equivocation = recovered(&w, PLACEMENT, message(&w, 1, b"different text", true));
    assert_eq!(
        inbox
            .accept(&recipient(&w), &provider_key(&w), &equivocation)
            .err(),
        Some(Error::Conflict)
    );
    assert_eq!(inbox.len(), 2);
}

// Every validation failure is refused with nothing remembered or decrypted: a foreign sender key, wrong size, wrong hash, a changed
// signature, a message for another recipient, a receipt for another placement, a forged provider signature or recipient authorization,
// the wrong provider key, and a payload that does not decrypt.
#[test]
fn every_validation_failure_is_refused_and_forgotten() {
    let w = world();
    let inbox = Inbox::new(8);
    let good = recovered(&w, PLACEMENT, message(&w, 1, b"hello", true));
    let mutate: Vec<Damage> = vec![
        (
            "sender key",
            Box::new(|o| o.message.sender_public_key = TestIdentity::new(9).key()),
        ),
        ("size", Box::new(|o| o.message.metadata.payload_size += 1)),
        (
            "hash",
            Box::new(|o| o.message.metadata.payload_hash.0[0] ^= 1),
        ),
        (
            "signature",
            Box::new(|o| o.message.sender_signature.0[0] ^= 1),
        ),
        ("payload", Box::new(|o| o.message.payload[40] ^= 1)),
        (
            "recipient",
            Box::new(|o| o.message.metadata.recipient_id = IdentityId([9; 32])),
        ),
        (
            "placement",
            Box::new(|o| o.placement_id = PlacementId([9; 32])),
        ),
        (
            "provider signature",
            Box::new(|o| o.receipt.provider_signature.0[0] ^= 1),
        ),
        (
            "recipient authorization",
            Box::new(|o| o.receipt.recipient_signature.0[0] ^= 1),
        ),
        ("recovered at", Box::new(|o| o.receipt.recovered_at += 1)),
    ];
    for (name, edit) in mutate {
        let mut bad = good.clone();
        edit(&mut bad);
        assert_eq!(
            inbox.accept(&recipient(&w), &provider_key(&w), &bad).err(),
            Some(Error::Invalid),
            "{name}"
        );
    }
    let wrong_provider = ProviderKey {
        id: w.provider.identity(),
        public_key: TestIdentity::new(9).key(),
    };
    assert_eq!(
        inbox.accept(&recipient(&w), &wrong_provider, &good).err(),
        Some(Error::Invalid),
        "provider key"
    );
    // A correctly signed message that was encrypted to another key authenticates but cannot be decrypted.
    let other = World {
        secret: Secret::new(Zeroizing::new([0x10; 32])),
        public: x25519_public(&[0x10; 32]),
        ..world()
    };
    let wrong_key = recovered(&w, PLACEMENT, message(&w, 3, b"for someone else", true));
    assert_eq!(
        inbox
            .accept(&recipient(&other), &provider_key(&w), &wrong_key)
            .err(),
        Some(Error::Invalid),
        "decryption"
    );
    assert!(inbox.is_empty(), "nothing failed was remembered");
    assert!(
        matches!(
            inbox.accept(&recipient(&w), &provider_key(&w), &good),
            Ok(Arrival::New(_))
        ),
        "the good message still works"
    );
}

// The inbox remembers at most its bound: a new message beyond it is refused, while a duplicate is still recognized.
#[test]
fn the_inbox_is_bounded() {
    let w = world();
    let inbox = Inbox::new(1);
    let first = recovered(&w, PLACEMENT, message(&w, 1, b"one", true));
    let second = recovered(&w, PLACEMENT, message(&w, 2, b"two", true));
    assert!(matches!(
        inbox.accept(&recipient(&w), &provider_key(&w), &first),
        Ok(Arrival::New(_))
    ));
    assert_eq!(
        inbox
            .accept(&recipient(&w), &provider_key(&w), &second)
            .err(),
        Some(Error::Limit)
    );
    assert!(matches!(
        inbox.accept(&recipient(&w), &provider_key(&w), &first),
        Ok(Arrival::Duplicate)
    ));
}
