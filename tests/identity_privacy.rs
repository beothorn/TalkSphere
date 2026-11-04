//! Trace-level capture of real protected Identity workflows and rejected private operations.
#![cfg(target_os = "linux")]
use std::sync::{Arc, Mutex};
use tracing::{
    Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};
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

// Deterministic private fixture material is confined to this test-only entropy source.
struct FixtureEntropy(u8);
impl talksphere::modules::cryptography::ports::Entropy for FixtureEntropy {
    // Independent recognizable fixture inputs make accidental key logging detectable.
    fn fill(
        &mut self,
        output: &mut [u8],
    ) -> Result<(), talksphere::modules::cryptography::error::CryptoError> {
        output.fill(self.0);
        self.0 += 1;
        Ok(())
    }
}

// Capture successful and rejected local workflows at TRACE, including key drops and public QR output.
#[test]
fn identity_workflows_keep_private_values_out_of_diagnostics() {
    use std::{fs, os::unix::fs::PermissionsExt};
    use talksphere::modules::{
        cryptography::{
            api::{EncryptRequest, RecipientCredential},
            config::ResourceLimits,
            envelope::encrypt,
            random::SystemEntropy,
        },
        identity::{
            api::IdentityProvider,
            authority::{Backup, Decrypt, Sign},
            config::Limits,
            operations::EnvelopeFormat,
            provider::Identity,
            storage::FileStore,
            text,
        },
    };
    let root = std::env::temp_dir().join(format!("talksphere-privacy-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let capture = Capture::default();
    tracing::subscriber::with_default(capture.clone(), || {
        let limits = Limits::new(256, 8, 4096, 8192, 4096).unwrap();
        let (store, admin) = FileStore::open(&root.join("source"), limits).unwrap();
        let mut provider = Identity::new(store, FixtureEntropy(81), limits, 1).unwrap();
        let (handle, public) = provider.create(&admin).unwrap();
        let signing = provider.grant::<Sign>(&admin, &handle, 4096).unwrap();
        provider
            .sign(&handle, &signing, b"PRIVATE_PROTOCOL_MESSAGE_MARKER")
            .unwrap();
        let qr = text::format(public.identity.id());
        assert!(qr.starts_with("cr2se:"));
        assert_eq!(qr.len(), 58);
        let recipient = RecipientCredential {
            identity_id: public.identity.id().0,
            signing_public_key: public.identity.signing_public_key(),
            encryption_public_key: public.encryption_public_key,
            binding_signature: public.binding_signature,
        };
        let mut object = encrypt::encrypt(
            EncryptRequest {
                plaintext: b"PRIVATE_PLAINTEXT_MARKER",
                recipient: &recipient,
                limits: ResourceLimits::new(4096, 8192).unwrap(),
            },
            &mut SystemEntropy,
        )
        .unwrap();
        let decrypt = provider.grant::<Decrypt>(&admin, &handle, 8192).unwrap();
        assert_eq!(
            provider
                .decrypt(&handle, &decrypt, EnvelopeFormat::GenericV1, &object)
                .unwrap()
                .as_slice(),
            b"PRIVATE_PLAINTEXT_MARKER"
        );
        *object.last_mut().unwrap() ^= 1;
        assert!(
            provider
                .decrypt(&handle, &decrypt, EnvelopeFormat::GenericV1, &object)
                .is_err()
        );
        let (mut backup, backup_admin) = FileStore::open(&root.join("backup"), limits).unwrap();
        let cap = provider.grant::<Backup>(&admin, &handle, 4096).unwrap();
        provider
            .backup(&handle, &cap, &mut backup, &backup_admin)
            .unwrap();
        assert!(
            provider
                .backup(&handle, &cap, &mut backup, &backup_admin)
                .is_err()
        );
        let (store, admin) = FileStore::open(&root.join("restored"), limits).unwrap();
        let mut restored = Identity::new(store, SystemEntropy, limits, 1).unwrap();
        let import = restored
            .grant_import(&admin, public.identity.id(), 4096)
            .unwrap();
        restored
            .restore(
                &admin,
                &import,
                public.identity.id(),
                &backup,
                &backup_admin,
            )
            .unwrap();
        let foreign = talksphere::modules::identity::authority::Authority::new()
            .administration(4096)
            .unwrap();
        assert!(backup.read(&foreign).is_err());
    });
    let logs = capture.0.lock().unwrap().join("\n");
    assert!(logs.contains("identity"));
    assert!(logs.contains("TRACE"));
    for forbidden in [
        "PRIVATE_PLAINTEXT_MARKER".into(),
        "PRIVATE_PROTOCOL_MESSAGE_MARKER".into(),
        "TSID0001".into(),
        "51".repeat(32),
        "52".repeat(32),
        "Q".repeat(32),
        "R".repeat(32),
        format!("{:?}", [81u8; 32]),
        format!("{:?}", [82u8; 32]),
    ] {
        assert!(!logs.contains(&forbidden));
    }
    fs::remove_dir_all(root).unwrap();
}
