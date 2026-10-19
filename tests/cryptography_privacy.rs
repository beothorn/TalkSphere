//! Public cryptography operations never expose private material through tracing.
use std::sync::{Arc, Mutex};
use talksphere::modules::cryptography::{
    api::{DecryptRequest, EncryptRequest, RecipientCredential},
    config::ResourceLimits,
    credential,
    envelope::{decrypt::decrypt, encrypt::encrypt},
    random::SystemEntropy,
    secrets::{Persistent, Secret, Signing},
};
use tracing::{
    Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};
use zeroize::Zeroizing;

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<String>>>);

impl Subscriber for Capture {
    // Record all codec events so a trace-level leak cannot hide behind a filter.
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    // These codecs emit events, but provide an ID if a span is added later.
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    // No test scenario updates span fields.
    fn record(&self, _: &Id, _: &Record<'_>) {}
    // No test scenario uses causal span links.
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    // Capture target, level and every field without imposing an application logger.
    fn event(&self, event: &Event<'_>) {
        let mut fields = Fields(format!(
            "{} {}",
            event.metadata().target(),
            event.metadata().level()
        ));
        event.record(&mut fields);
        self.0.lock().unwrap().push(fields.0);
    }
    // Entry does not affect event content in this recording subscriber.
    fn enter(&self, _: &Id) {}
    // Exit does not affect event content in this recording subscriber.
    fn exit(&self, _: &Id) {}
}

struct Fields(String);
impl Visit for Fields {
    // Include field names as well as values when checking for leaked peer data.
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write;
        write!(self.0, " {}={value:?}", field.name()).unwrap();
    }
}

// Capture a real public encryption/decryption workflow, including authentication rejection and drops.
#[test]
fn cryptographic_traces_exclude_plaintext_and_private_material() {
    let capture = Capture::default();
    tracing::subscriber::with_default(capture.clone(), || {
        let signing = Secret::<Signing>::new(Zeroizing::new([23; 32]));
        let private = Secret::<Persistent>::new(Zeroizing::new([24; 32]));
        let signing_public_key = ed25519_dalek::SigningKey::from_bytes(&[23; 32])
            .verifying_key()
            .to_bytes();
        let encryption_public_key =
            x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from([24; 32])).to_bytes();
        let recipient = RecipientCredential {
            identity_id: credential::identity_id(&signing_public_key).unwrap(),
            signing_public_key,
            encryption_public_key,
            binding_signature: credential::bind(&signing, &encryption_public_key).unwrap(),
        };
        let limits = ResourceLimits::new(1024, 1172).unwrap();
        let plaintext = b"DO_NOT_LOG_PRIVATE_APPLICATION_CONTENT";
        let mut object = encrypt(
            EncryptRequest {
                plaintext,
                recipient: &recipient,
                limits,
            },
            &mut SystemEntropy,
        )
        .unwrap();
        assert_eq!(
            decrypt(DecryptRequest {
                object: &object,
                identity_id: recipient.identity_id,
                encryption_public_key,
                private_key: &private,
                limits
            })
            .unwrap(),
            plaintext
        );
        *object.last_mut().unwrap() ^= 1;
        assert!(
            decrypt(DecryptRequest {
                object: &object,
                identity_id: recipient.identity_id,
                encryption_public_key,
                private_key: &private,
                limits
            })
            .is_err()
        );
    });
    let logs = capture.0.lock().unwrap().join("\n");
    assert!(logs.contains("cryptography"));
    assert!(logs.contains("TRACE"));
    for forbidden in [
        "DO_NOT_LOG_PRIVATE_APPLICATION_CONTENT".to_string(),
        "17".repeat(32),
        "18".repeat(32),
        format!("{:?}", [23u8; 32]),
        format!("{:?}", [24u8; 32]),
    ] {
        assert!(!logs.contains(&forbidden));
    }
}
