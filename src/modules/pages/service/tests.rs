//! Page provider adapter, client and local-check tests through the Services handler contract: PG-02 to PG-12.
use super::{
    client::Client,
    codec::{self, GetRequest},
    provider::PageProvider,
    standard::Catalogue,
    store::{MemoryPages, Pages, Snapshot},
    terms::tests::offering,
};
use crate::modules::{
    hosted_publications::journal::Journal,
    identity::api::IdentityId,
    messaging::test_support::SequenceEntropy,
    pages::{error::Error as PageError, media::MediaType, path::CanonicalPath},
    services::{
        Error,
        contract::OperationKey,
        handler::{Blob, Check, Invocation, Peer, Provider as _, Requester as _, Status},
        pricing::Price,
        value::{Budget, Value},
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const BUDGET: Budget = Budget {
    depth: 8,
    nodes: 4096,
    bytes: 65536,
};
const HOST: IdentityId = IdentityId([2; 32]);
const VISITOR: IdentityId = IdentityId([5; 32]);

fn path(text: &str) -> CanonicalPath {
    CanonicalPath::parse(text.as_bytes()).unwrap()
}
fn key(n: u8) -> OperationKey {
    OperationKey {
        requester: VISITOR,
        connection: [n; 32],
        stream: u32::from(n),
    }
}
fn peer(n: u8) -> Peer {
    Peer {
        local: HOST,
        remote: VISITOR,
        key: key(n),
    }
}
fn request(text: &str, maximum: u64) -> Value {
    codec::request_value(&GetRequest {
        path: path(text),
        maximum_bytes: maximum,
    })
}

// A page source that counts how often it is consulted, to show invalid requests never reach it.
struct Counting {
    inner: MemoryPages,
    reads: AtomicUsize,
}
impl Pages for Counting {
    fn snapshot(&self, p: &CanonicalPath) -> Result<Option<Snapshot>, PageError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.inner.snapshot(p)
    }
}

struct Rig {
    pages: Arc<Counting>,
    provider: PageProvider,
    journal_dir: std::path::PathBuf,
}

fn rig() -> Rig {
    use std::sync::atomic::AtomicU32;
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let journal_dir = std::env::temp_dir().join(format!(
        "ts-pages-journal-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let pages = Arc::new(Counting {
        inner: MemoryPages::default(),
        reads: AtomicUsize::new(0),
    });
    pages.inner.publish(
        CanonicalPath::root(),
        MediaType::parse("text/html").unwrap(),
        b"abc".to_vec(),
    );
    let provider = provider_over(pages.clone(), &journal_dir);
    Rig {
        pages,
        provider,
        journal_dir,
    }
}

fn provider_over(pages: Arc<Counting>, dir: &std::path::Path) -> PageProvider {
    PageProvider::new(
        HOST,
        pages,
        Journal::open(dir, 64).unwrap(),
        Arc::new(SequenceEntropy::new(7)),
        BUDGET,
        65536,
    )
    .unwrap()
}

fn invocation(n: u8) -> Invocation {
    let offering = offering();
    let definition = Catalogue::new(Default::default())
        .unwrap()
        .definition(&offering)
        .unwrap();
    let accepted = Blob::new(b"accepted".to_vec(), 64).unwrap();
    Invocation {
        key: key(n),
        requester: VISITOR,
        provider: HOST,
        accepted: accepted.clone(),
        maximum: 4,
        publisher: HOST,
        issuer: offering.issuer,
        offering,
        definition,
        quote: accepted,
        factors: Value::Object(vec![]),
        pricing: Price::Fixed(4),
    }
}

// Run a get through assess, execute, prepare and commit and return its staged completion.
fn run(
    rig: &Rig,
    n: u8,
    input: &Value,
) -> Result<crate::modules::services::handler::Completion, Error> {
    rig.provider.assess(peer(n), &offering(), input)?;
    let completion = rig.provider.execute(&invocation(n), input)?;
    let token = rig.provider.prepare(key(n), &completion.effect)?;
    rig.provider.commit(key(n), &token)?;
    Ok(completion)
}

// PG-02: a get of the root with maximumBytes 3 returns the exact path, media type, size, SHA-256 of `abc`, content and the fixed price 4.
#[test]
fn a_get_returns_the_exact_output_at_the_fixed_price() {
    let rig = rig();
    let done = run(&rig, 1, &request("/", 3)).unwrap();
    assert_eq!(done.price, 4);
    let output = codec::parse_output(&done.output).unwrap();
    assert_eq!(
        (
            output.path.as_str(),
            output.media_type.as_str(),
            output.content_size,
            output.content.as_slice(),
            output.price
        ),
        ("/", "text/html", 3, &b"abc"[..], 4)
    );
    let hash: String = output
        .content_hash
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        hash,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert!(matches!(rig.provider.status(key(1)), Ok(Status::Committed)));
}

// PG-03: an invalid path never reaches the resource source, is refused without any staged work, and cost nothing; a path above the offering's
// path limit is refused the same way.
#[test]
fn invalid_paths_are_refused_before_any_access() {
    let rig = rig();
    let bad = |text: &str| {
        Value::Object(vec![
            ("path".into(), Value::String(text.into())),
            ("maximumBytes".into(), Value::Uint64(3)),
        ])
    };
    let too_long = format!("/{}", "a/".repeat(600).trim_end_matches('/'));
    for text in [
        "about",
        "/about/",
        "//a",
        "/a/./b",
        "/a/../b",
        "/images\\photo.jpg",
        "/a\u{1}b",
        &too_long,
    ] {
        assert_eq!(
            rig.provider.assess(peer(1), &offering(), &bad(text)).err(),
            Some(Error::Invalid),
            "{text:?}"
        );
        assert_eq!(
            rig.provider.execute(&invocation(1), &bad(text)).err(),
            Some(Error::Invalid),
            "{text:?}"
        );
    }
    assert_eq!(
        rig.pages.reads.load(Ordering::SeqCst),
        0,
        "the source is never read for an invalid request"
    );
    assert!(matches!(rig.provider.status(key(1)), Ok(Status::Absent)));
}

// PG-05: a byte bound below the content, above the offering or zero fails; the exact bound succeeds; an empty resource has size zero and the empty
// SHA-256 and still costs the fixed price.
#[test]
fn bounds_and_empty_resources() {
    let rig = rig();
    assert_eq!(run(&rig, 1, &request("/", 2)).err(), Some(Error::Limit));
    assert_eq!(
        rig.provider
            .assess(peer(2), &offering(), &request("/", 101))
            .err(),
        Some(Error::Invalid)
    );
    assert_eq!(
        rig.provider
            .assess(peer(2), &offering(), &request("/", 0))
            .err(),
        Some(Error::Invalid)
    );
    assert!(run(&rig, 3, &request("/", 3)).is_ok());
    rig.pages.inner.publish(
        path("/empty"),
        MediaType::parse("text/plain").unwrap(),
        vec![],
    );
    let done = run(&rig, 4, &request("/empty", 100)).unwrap();
    let output = codec::parse_output(&done.output).unwrap();
    let hash: String = output
        .content_hash
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!((output.content_size, done.price), (0, 4));
    assert_eq!(
        hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    // A resource above the offering's own limit fails even when the visitor would accept it.
    rig.pages.inner.publish(
        path("/big"),
        MediaType::parse("text/plain").unwrap(),
        vec![1; 101],
    );
    assert_eq!(
        run(&rig, 5, &request("/big", 100)).err(),
        Some(Error::Limit)
    );
}

// PG-06: a missing resource fails as a concealed denial, never as the root or an error page; a failed get leaves no prepared work.
#[test]
fn a_missing_resource_is_a_failure_not_a_substitute() {
    let rig = rig();
    assert_eq!(
        run(&rig, 1, &request("/missing", 100)).err(),
        Some(Error::Denied)
    );
    assert!(matches!(rig.provider.status(key(1)), Ok(Status::Absent)));
}

// PG-01: the offering is served only while the root is usable: after the root is removed, replaced by a non-HTML resource or grows above the
// offering's limit, requests are unavailable; the page works again once a valid root returns.
#[test]
fn an_unusable_root_stops_service() {
    let rig = rig();
    assert!(
        rig.provider
            .assess(peer(1), &offering(), &request("/", 3))
            .is_ok()
    );
    rig.pages.inner.remove(&CanonicalPath::root());
    assert_eq!(
        rig.provider
            .assess(peer(1), &offering(), &request("/", 3))
            .err(),
        Some(Error::Unavailable)
    );
    rig.pages.inner.publish(
        CanonicalPath::root(),
        MediaType::parse("text/plain").unwrap(),
        b"abc".to_vec(),
    );
    assert_eq!(
        rig.provider
            .assess(peer(1), &offering(), &request("/", 3))
            .err(),
        Some(Error::Unavailable)
    );
    rig.pages.inner.publish(
        CanonicalPath::root(),
        MediaType::parse("text/html").unwrap(),
        vec![1; 101],
    );
    assert_eq!(
        rig.provider
            .assess(peer(1), &offering(), &request("/", 3))
            .err(),
        Some(Error::Unavailable)
    );
    rig.pages.inner.publish(
        CanonicalPath::root(),
        MediaType::parse("text/html").unwrap(),
        b"abc".to_vec(),
    );
    assert!(
        rig.provider
            .assess(peer(1), &offering(), &request("/", 3))
            .is_ok()
    );
}

// PG-07/PG-08: the staged result is pinned: replacing the root afterwards does not change what is committed, a new explicit get sees the new
// bytes at a new charge, and two visitors see the same content, selected only by the path.
#[test]
fn staged_results_are_pinned_snapshots() {
    let rig = rig();
    let old = rig
        .provider
        .execute(&invocation(1), &request("/", 100))
        .unwrap();
    rig.pages.inner.publish(
        CanonicalPath::root(),
        MediaType::parse("text/html").unwrap(),
        b"new!".to_vec(),
    );
    assert_eq!(rig.provider.completion(key(1)).unwrap().output, old.output);
    assert_eq!(codec::parse_output(&old.output).unwrap().content, b"abc");
    let token = rig.provider.prepare(key(1), &old.effect).unwrap();
    rig.provider.commit(key(1), &token).unwrap();
    let fresh = run(&rig, 2, &request("/", 100)).unwrap();
    assert_eq!(codec::parse_output(&fresh.output).unwrap().content, b"new!");
    assert_eq!(fresh.price, 4);
    // Another visitor under another key gets the same selection.
    let other = Peer {
        local: HOST,
        remote: IdentityId([6; 32]),
        key: OperationKey {
            requester: IdentityId([6; 32]),
            connection: [9; 32],
            stream: 9,
        },
    };
    let mut invoked = invocation(9);
    invoked.key = other.key;
    invoked.requester = other.remote;
    let same = rig.provider.execute(&invoked, &request("/", 100)).unwrap();
    assert_eq!(same.output, fresh.output);
}

// PG-10: a restart keeps the journal: a staged-but-uncommitted get is rebuilt from the journaled request and terms and a lost reply is answered from the
// committed status; an explicit abort releases the staged work; an unknown key is unknown.
#[test]
fn staging_survives_a_restart() {
    let rig = rig();
    let input = request("/", 100);
    let staged = rig.provider.execute(&invocation(1), &input).unwrap();
    let done = run(&rig, 2, &input).unwrap();
    drop(rig.provider);
    let restarted = provider_over(rig.pages.clone(), &rig.journal_dir);
    let rebuilt = restarted.completion(key(1)).unwrap();
    assert_eq!(
        (rebuilt.output, rebuilt.price),
        (staged.output, staged.price)
    );
    assert!(matches!(restarted.status(key(2)), Ok(Status::Committed)));
    assert_eq!(restarted.completion(key(9)).err(), Some(Error::Unknown));
    let token = restarted.prepare(key(1), &rebuilt.effect).unwrap();
    assert_eq!(
        restarted
            .commit(key(1), &Blob::new(vec![0; 32], 32).unwrap())
            .err(),
        Some(Error::Denied)
    );
    restarted.commit(key(1), &token).unwrap();
    restarted.commit(key(1), &token).unwrap();
    assert_eq!(restarted.abort(key(1), None).err(), Some(Error::Conflict));
    let again = restarted.execute(&invocation(3), &input).unwrap();
    restarted.abort(key(3), None).unwrap();
    assert!(matches!(restarted.status(key(3)), Ok(Status::Aborted)));
    assert_eq!(
        restarted
            .commit(key(3), &Blob::new(vec![0; 32], 32).unwrap())
            .err(),
        Some(Error::Conflict)
    );
    let _ = (done, again);
}

// Assessment serves only the publisher's own identity, only the authenticated remote's own operations and only a provided offering; the Page has
// no remote check to serve.
#[test]
fn assessment_is_bound_to_the_publisher_and_the_visitor() {
    let rig = rig();
    let input = request("/", 3);
    let good = peer(1);
    assert!(rig.provider.assess(good, &offering(), &input).is_ok());
    assert_eq!(
        rig.provider
            .assess(
                Peer {
                    local: IdentityId([9; 32]),
                    ..good
                },
                &offering(),
                &input
            )
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        rig.provider
            .assess(
                Peer {
                    remote: IdentityId([9; 32]),
                    ..good
                },
                &offering(),
                &input
            )
            .err(),
        Some(Error::Denied)
    );
    let mut wanted = offering();
    wanted.direction = crate::modules::board::offering::Direction::Wanted;
    assert_eq!(
        rig.provider.assess(good, &wanted, &input).err(),
        Some(Error::Denied)
    );
    let mut bad_terms = offering();
    bad_terms.source["price"] = 0.into();
    assert_eq!(
        rig.provider.assess(good, &bad_terms, &input).err(),
        Some(Error::Invalid)
    );
    // A different offering ID or description is irrelevant: the contract is the service and version.
    assert_eq!(
        PageProvider::new(
            HOST,
            rig.pages.clone(),
            Journal::volatile(4),
            Arc::new(SequenceEntropy::new(1)),
            Budget { depth: 0, ..BUDGET },
            10
        )
        .err(),
        Some(Error::Invalid)
    );
}

// The client validates the fixed price offering, refuses a result that does not belong to its request or fails the local check, and computes the
// local check outcome itself from its own request, the agreed price and the result (PG-09), never from a claim of the publisher.
#[test]
fn the_client_checks_results_locally() {
    let rig = rig();
    let catalogue = Catalogue::new(Default::default()).unwrap();
    let definition = catalogue.definition(&offering()).unwrap();
    let client = Client::new(definition, &catalogue, BUDGET).unwrap();
    client.terms(&offering()).unwrap();
    assert_eq!(
        client.pricing(&offering(), &Value::Object(vec![])),
        Ok(Price::Fixed(4))
    );
    let input = request("/", 3);
    let done = run(&rig, 1, &input).unwrap();
    let invoked = invocation(1);
    client
        .validate_result(&invoked, &input, &done.output)
        .unwrap();
    let terms = Blob::new(b"terms".to_vec(), 16).unwrap();
    let agreed = Value::Object(vec![("agreedPrice".into(), Value::Uint64(4))]);
    assert!(matches!(
        client.verify(&input, &terms, &done.output, Some(&agreed)),
        Ok(Check::Pass)
    ));
    // Missing expectation: inconclusive, never a pass.
    assert!(matches!(
        client.verify(&input, &terms, &done.output, None),
        Ok(Check::Inconclusive)
    ));
    let wrong = Value::Object(vec![("agreedPrice".into(), Value::Uint64(5))]);
    assert!(matches!(
        client.verify(&input, &terms, &done.output, Some(&wrong)),
        Ok(Check::Fail)
    ));
    let edit = |name: &str, value: Value| {
        let Value::Object(fields) = done.output.clone() else {
            unreachable!()
        };
        Value::Object(
            fields
                .into_iter()
                .map(|(n, v)| {
                    if n == name {
                        (n, value.clone())
                    } else {
                        (n, v)
                    }
                })
                .collect(),
        )
    };
    // A different path, a lying hash, a lying size, another price and modified content are refused as results and fail the check.
    let tampered = [
        edit("path", Value::String("/other".into())),
        edit("contentHash", Value::Bytes(vec![0; 32])),
        edit("contentSize", Value::Uint64(2)),
        edit("price", Value::Uint64(3)),
        edit("content", Value::Bytes(b"abd".to_vec())),
    ];
    for output in tampered {
        assert!(client.validate_result(&invoked, &input, &output).is_err());
        assert!(!matches!(
            client.verify(&input, &terms, &output, Some(&agreed)),
            Ok(Check::Pass)
        ));
    }
    // An invalid label fails the check even though the value cannot be decoded as an output.
    let bad_label = edit("mediaType", Value::String("Text/HTML".into()));
    assert!(matches!(
        client.verify(&input, &terms, &bad_label, Some(&agreed)),
        Ok(Check::Fail)
    ));
    assert!(
        client
            .validate_result(&invoked, &input, &bad_label)
            .is_err()
    );
    // A result above the visitor's own bound is refused.
    assert!(
        client
            .validate_result(&invoked, &request("/", 2), &done.output)
            .is_err()
    );
    // The outcome names are exactly pass, fail and inconclusive and nothing else is a valid check output.
    use crate::modules::services::validation::Semantics;
    let definition_value = client.definition().document.value().clone();
    for (name, ok) in [
        ("pass", true),
        ("fail", true),
        ("inconclusive", true),
        ("passed", false),
    ] {
        let value = Value::Object(vec![("outcome".into(), Value::String(name.into()))]);
        assert_eq!(
            client
                .validate(&definition_value["check"]["output"], &value)
                .is_ok(),
            ok,
            "{name}"
        );
    }
}
