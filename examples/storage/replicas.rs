//! Encryption and replacement-device recovery over independently authenticated paid leases.
use super::*;
use std::{fs, io::Write, os::unix::fs::OpenOptionsExt, path::Path};
use talksphere::modules::{
    cryptography::{
        self,
        api::{EncryptRequest, RecipientCredential},
        random::SystemEntropy,
    },
    identity::{
        self,
        api::IdentityProvider,
        authority::{Backup, Decrypt},
        operations::EnvelopeFormat,
    },
};

// Backup includes both protected identity keys. Encryption uses only the validated public credential.
fn encrypt_and_backup(
    original: &Path,
    backup: &Path,
    plaintext: &[u8],
) -> Result<(IdentityId, Vec<u8>), String> {
    let limits = identity::config::Limits::new(256, 8, 4096, 8192, 4096).map_err(failure)?;
    let (source, admin) = identity::storage::FileStore::open(original, limits).map_err(failure)?;
    let mut identity =
        identity::provider::Identity::new(source, SystemEntropy, limits, 1).map_err(failure)?;
    let (handle, public) = identity.create(&admin).map_err(failure)?;
    let cap = identity
        .grant::<Backup>(&admin, &handle, 4096)
        .map_err(failure)?;
    let (mut protected, protected_admin) =
        identity::storage::FileStore::open(backup, limits).map_err(failure)?;
    identity
        .backup(&handle, &cap, &mut protected, &protected_admin)
        .map_err(failure)?;
    let id = public.identity.id();
    let recipient = RecipientCredential {
        identity_id: id.0,
        signing_public_key: public.identity.signing_public_key(),
        encryption_public_key: public.encryption_public_key,
        binding_signature: public.binding_signature,
    };
    let object = cryptography::envelope::encrypt::encrypt(
        EncryptRequest {
            plaintext,
            recipient: &recipient,
            limits: cryptography::config::ResourceLimits::new(8192, 8192).map_err(failure)?,
        },
        &mut SystemEntropy,
    )
    .map_err(failure)?;
    Ok((id, object))
}

// Restore the original identity into a fresh protected store without consulting a provider.
fn restore(backup: &Path, destination: &Path, expected: IdentityId) -> Result<(), String> {
    let limits = identity::config::Limits::new(256, 8, 4096, 8192, 4096).map_err(failure)?;
    let (source, source_admin) =
        identity::storage::FileStore::open(backup, limits).map_err(failure)?;
    let (store, admin) =
        identity::storage::FileStore::open(destination, limits).map_err(failure)?;
    let mut identity =
        identity::provider::Identity::new(store, SystemEntropy, limits, 1).map_err(failure)?;
    let cap = identity
        .grant_import(&admin, expected, 4096)
        .map_err(failure)?;
    let (_, public) = identity
        .restore(&admin, &cap, expected, &source, &source_admin)
        .map_err(failure)?;
    assert_eq!(public.identity.id(), expected);
    Ok(())
}

// An authorized decryption operation authenticates the exact retrieved generic envelope.
fn decrypt(restored: &Path, object: &[u8]) -> Result<zeroize::Zeroizing<Vec<u8>>, String> {
    let limits = identity::config::Limits::new(256, 8, 4096, 8192, 4096).map_err(failure)?;
    let (store, admin) = identity::storage::FileStore::open(restored, limits).map_err(failure)?;
    let mut identity =
        identity::provider::Identity::new(store, SystemEntropy, limits, 1).map_err(failure)?;
    let (handle, _) = identity.load(&admin).map_err(failure)?;
    let cap = identity
        .grant::<Decrypt>(&admin, &handle, 8192)
        .map_err(failure)?;
    identity
        .decrypt(&handle, &cap, EnvelopeFormat::GenericV1, object)
        .map_err(failure)
}

// One committed receipt is retained under its actual provider identity and exact opaque lease ID.
fn purchase(network: &mut Network<'_>, ciphertext: &[u8]) -> Result<Lease, String> {
    let size = ciphertext.len() as u64;
    let duration = 90;
    let hash =
        cryptography::primitives::hash::sha256(ciphertext, ciphertext.len()).map_err(failure)?;
    let input = fields([
        ("operation", Value::String("store".into())),
        ("sizeBytes", Value::Uint64(size)),
        ("retentionSeconds", Value::Uint64(duration)),
        ("contentHash", Value::Bytes(hash.to_vec())),
        ("data", Value::Bytes(ciphertext.to_vec())),
    ]);
    let key = network.invoke(input.clone(), (size * (1 + duration)).div_ceil(2))?;
    let grant = network
        .owner
        .services
        .grant(
            network.requester_id,
            services::authority::Action::Observe,
            16_777_216,
        )
        .map_err(failure)?;
    let context = services::contract::Context {
        version: 1,
        grant: &grant,
        deadline: Instant::now() + Duration::from_secs(5),
        cancelled: false,
    };
    let (invocation, result) = network
        .owner
        .runtime
        .completed(&context, key)
        .map_err(failure)?;
    let observed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(failure)?
        .as_secs();
    talksphere::modules::storage::requester::store::validate(
        &invocation,
        &input,
        &result.output,
        owners::BUDGET,
        talksphere::modules::storage::requester::store::Policy {
            observed_unix_seconds: observed,
            maximum_clock_difference_seconds: 60,
        },
    )
    .map_err(failure)
}

// Save independent local lease metadata only after full authenticated result validation.
fn save(path: &Path, leases: &[Lease]) -> Result<(), String> {
    use talksphere::modules::identity::text;
    let records = leases.iter().map(|l| serde_json::json!({
        "id": l.id, "requester": text::format(l.requester), "provider": text::format(l.provider),
        "size": l.size_bytes, "hash": l.content_hash, "start": l.accepted_at,
        "end": l.expires_at, "price": l.store_price, "terms": l.accepted_terms.bytes()
    })).collect::<Vec<_>>();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(failure)?;
    file.write_all(&serde_json::to_vec(&records).map_err(failure)?)
        .map_err(failure)?;
    file.sync_all().map_err(failure)?;
    fs::File::open(path.parent().ok_or("Missing catalog parent")?)
        .and_then(|p| p.sync_all())
        .map_err(failure)
}

// Recover the exact original provider-scoped lease from the independently saved application catalog.
fn load(path: &Path, selected: IdentityId) -> Result<Lease, String> {
    use talksphere::modules::identity::text;
    let records: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(path).map_err(failure)?).map_err(failure)?;
    let provider = text::format(selected);
    let lease = records
        .iter()
        .find(|l| l["provider"].as_str() == Some(&provider))
        .ok_or("Catalog has no lease at selected provider")?;
    let amount = |name: &str| lease[name].as_u64().ok_or("Invalid local catalog amount");
    Ok(Lease {
        id: serde_json::from_value(lease["id"].clone()).map_err(failure)?,
        requester: text::parse(
            lease["requester"]
                .as_str()
                .ok_or("Invalid catalog requester")?,
        )
        .map_err(failure)?,
        provider: selected,
        size_bytes: amount("size")?,
        content_hash: serde_json::from_value(lease["hash"].clone()).map_err(failure)?,
        accepted_at: amount("start")?,
        expires_at: amount("end")?,
        store_price: amount("price")?,
        state: State::Active,
        accepted_terms: services::handler::Blob::new(
            serde_json::from_value(lease["terms"].clone()).map_err(failure)?,
            65536,
        )
        .map_err(failure)?,
    })
}

// A replacement requester recovers and decrypts solely through C while B and all of B's leases are offline.
#[test]
fn encrypted_replica_recovers_with_one_provider_offline() {
    run().unwrap();
}

// Drive the public identity, crypto and paid peer APIs against independent temporary owners.
fn run() -> Result<(), String> {
    use std::collections::BTreeSet;
    let root = Directory::new()?;
    let original = root.0.join("original");
    let restored = root.0.join("restored");
    let backup = root.0.join("backup");
    let a_dir = root.0.join("a");
    let b_dir = root.0.join("b");
    let c_dir = root.0.join("c");
    let b_identity = root.0.join("b-identity");
    let c_identity = root.0.join("c-identity");
    let catalog_path = root.0.join("catalog");
    let plaintext = b"independent durable Storage replica";
    let (aid, ciphertext) = encrypt_and_backup(&original, &backup, plaintext)?;
    let (bid, cid) = (node::identity(&b_identity)?, node::identity(&c_identity)?);
    assert_ne!(bid, cid);
    let signal = Signal::default();
    let a_owner = owners::open(
        &a_dir,
        aid,
        bid,
        true,
        true,
        Arc::new(Catalog(Mutex::new(None))),
    )?;
    // Explicitly provide C's currency, separately from B's.
    let grant = a_owner
        .ledger_authority
        .grant(
            aid,
            talksphere::modules::ledger::authority::Action::Administer,
            16_777_216,
        )
        .map_err(failure)?;
    a_owner
        .account
        .set_owned(
            &talksphere::modules::ledger::contract::Context {
                version: 1,
                grant: &grant,
                deadline: Instant::now() + Duration::from_secs(10),
                cancelled: false,
            },
            cid,
            1_000_000,
        )
        .map_err(failure)?;
    let mut a = Node::new(&original, a_owner.endpoint.clone(), signal.clone())?;
    let mut b = Host::start(&b_dir, &b_identity, bid, aid, true, signal.clone(), false)?;
    let mut c = Host::start(&c_dir, &c_identity, cid, aid, true, signal.clone(), false)?;
    let (connection, cache) = connect(&mut a, &mut b, &a_owner, bid)?;
    let mut network_b = Network {
        requester: &mut a,
        provider: &mut b,
        owner: &a_owner,
        connection,
        cache,
        provider_id: bid,
        requester_id: aid,
    };
    let first = purchase(&mut network_b, &ciphertext)
        .map_err(|e| format!("first lease at B failed. {e}"))?;
    let second = purchase(&mut network_b, &ciphertext)
        .map_err(|e| format!("second lease at B failed. {e}"))?;
    assert_ne!(first.id, second.id);
    assert_eq!(BTreeSet::from([first.provider, second.provider]).len(), 1);
    let (connection, cache) =
        connect(&mut a, &mut c, &a_owner, cid).map_err(|e| format!("connect at C failed. {e}"))?;
    let mut network_c = Network {
        requester: &mut a,
        provider: &mut c,
        owner: &a_owner,
        connection,
        cache,
        provider_id: cid,
        requester_id: aid,
    };
    let independent =
        purchase(&mut network_c, &ciphertext).map_err(|e| format!("lease at C failed. {e}"))?;
    assert_eq!(
        BTreeSet::from([first.provider, second.provider, independent.provider]).len(),
        2
    );
    save(&catalog_path, &[first, second, independent])?;
    drop(ciphertext);
    b.stop()?;
    c.stop()?;
    drop(a);
    drop(a_owner);
    fs::remove_dir_all(&original).map_err(failure)?;
    restore(&backup, &restored, aid).map_err(|e| format!("identity restore failed. {e}"))?;
    let lease = load(&catalog_path, cid)?;
    let a_owner = owners::open(
        &a_dir,
        aid,
        cid,
        true,
        false,
        Arc::new(Catalog(Mutex::new(Some(lease.clone())))),
    )?;
    let mut a = Node::new(&restored, a_owner.endpoint.clone(), signal.clone())?;
    let mut c = Host::start(&c_dir, &c_identity, cid, aid, false, signal, false)?;
    let (connection, cache) = connect(&mut a, &mut c, &a_owner, cid)?;
    let mut network = Network {
        requester: &mut a,
        provider: &mut c,
        owner: &a_owner,
        connection,
        cache,
        provider_id: cid,
        requester_id: aid,
    };
    let key = network.invoke(
        fields([
            ("operation", Value::String("retrieve".into())),
            ("leaseId", Value::Bytes(lease.id.to_vec())),
            ("offset", Value::Uint64(0)),
            ("length", Value::Uint64(lease.size_bytes)),
        ]),
        lease.size_bytes.div_ceil(2),
    )?;
    let returned = network.result(key)?;
    let Value::Bytes(object) = field(&returned.output, "data")? else {
        return Err("Missing encrypted replica bytes".into());
    };
    assert_eq!(
        cryptography::primitives::hash::sha256(object, object.len()).map_err(failure)?,
        lease.content_hash
    );
    c.stop()?;
    drop(a);
    assert_eq!(
        decrypt(&restored, object)
            .map_err(|e| format!("decrypt restored object failed. {e}"))?
            .as_slice(),
        plaintext
    );
    Ok(())
}
