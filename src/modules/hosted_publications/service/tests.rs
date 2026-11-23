//! Services provider adapter tests, driven through the Services handler contract the coordinator uses: assess, stage, prepare, finalize,
//! commit, status, restart resumption, cached retry and evidence.
use super::provider::ServiceProvider;
use crate::modules::{
    hosted_publications::{
        codec::{self, Request, Response},
        ids::{PublicationId, RemovalId},
        journal::Journal,
        metadata::{Kind, Publication},
        operation::{
            CheckRequest, GetRequest, ListRequest, RemoveRequest, RenewRequest, StoreRequest,
        },
        participant::Participant,
        provider::{Provider, tests::public},
        signing::tests::VectorPublisher,
        standard::Catalogue,
        store::{Limits, Store},
        terms::tests::{offering, terms},
    },
    identity::api::IdentityId,
    messaging::{
        ports::{FixedLimits, Signer},
        test_support::{FixedClock, RecordingIssuer, SequenceEntropy},
    },
    services::{
        Error,
        checks::Invocation as CheckInvocation,
        contract::OperationKey,
        handler::{Blob, Completion, Invocation, Peer, Provider as _, Status},
        input::Input,
        pricing::Price,
        value::{Budget, Value},
    },
};
use std::sync::Arc;

const BUDGET: Budget = Budget {
    depth: 8,
    nodes: 4096,
    bytes: 65536,
};
const HOST: IdentityId = IdentityId([2; 32]);

fn key(n: u8, requester: IdentityId) -> OperationKey {
    OperationKey {
        requester,
        connection: [n; 32],
        stream: u32::from(n),
    }
}

// A rig: a participant over volatile state with a clock at 1000, ready for work, and the vector publisher as requester.
struct Rig {
    participant: Arc<Participant>,
    clock: Arc<FixedClock>,
    issuer: Arc<RecordingIssuer>,
    publisher: VectorPublisher,
}

fn rig() -> Rig {
    let clock = Arc::new(FixedClock::at(1000));
    let issuer = Arc::new(RecordingIssuer::default());
    let provider = Provider::new(
        terms(),
        Arc::new(Store::volatile(Limits {
            placements: 16,
            content_bytes: 1000,
        })),
        clock.clone(),
        Arc::new(SequenceEntropy::new(10)),
        Arc::new(FixedLimits::default()),
    )
    .with_release_policy(Arc::new(|payments: u64, _: u64| payments * 30 / 100))
    .with_release_issuer(issuer.clone());
    let participant = Arc::new(Participant::new(
        Arc::new(provider),
        Journal::volatile(16),
        Arc::new(SequenceEntropy::new(500)),
    ));
    participant
        .recover(&crate::modules::hosted_publications::recovery::DeferToServices)
        .unwrap();
    Rig {
        participant,
        clock,
        issuer,
        publisher: VectorPublisher::new(),
    }
}

impl Rig {
    fn adapter(&self) -> ServiceProvider {
        ServiceProvider::new(self.participant.clone(), BUDGET, 65536).unwrap()
    }
    fn me(&self) -> IdentityId {
        self.publisher.identity()
    }
    // An invocation by `requester` under `key`.
    fn invocation(&self, key: OperationKey, requester: IdentityId) -> Invocation {
        let offering = offering();
        let catalogue = Catalogue::new(Default::default()).unwrap();
        let definition = catalogue.definition(&offering).unwrap();
        let accepted = Blob::new(b"accepted".to_vec(), 64).unwrap();
        Invocation {
            key,
            requester,
            provider: HOST,
            accepted: accepted.clone(),
            maximum: 1000,
            publisher: HOST,
            issuer: offering.issuer,
            offering,
            definition,
            quote: accepted,
            factors: Value::Object(vec![]),
            pricing: Price::Fixed(1),
        }
    }
    fn peer(&self, key: OperationKey, requester: IdentityId) -> Peer {
        Peer {
            local: HOST,
            remote: requester,
            key,
        }
    }
}

// Run a store through the whole staged path and return its finalized completion.
fn stored(
    rig: &Rig,
    n: u8,
    kind: Kind,
    sequence: u64,
    content: &[u8],
) -> (Publication, Completion, OperationKey) {
    let adapter = rig.adapter();
    let publication = public(&rig.publisher, n, kind, sequence, content);
    let input = codec::request_value(&Request::Store(StoreRequest {
        publication: publication.clone(),
        retention_seconds: 10,
    }));
    let k = key(n, rig.me());
    let invocation = rig.invocation(k, rig.me());
    let template = adapter.execute(&invocation, &input).unwrap();
    let token = adapter.prepare(k, &template.effect).unwrap();
    let finalized = adapter.finalize(k, &token, &template).unwrap();
    adapter
        .commit_finalized(k, &token, &template, &finalized)
        .unwrap();
    (publication, finalized, k)
}

// Execute a read-only or single-step operation and commit it as prepared.
fn run(rig: &Rig, n: u8, requester: IdentityId, request: &Request) -> Result<Completion, Error> {
    let adapter = rig.adapter();
    let input = codec::request_value(request);
    let k = key(n, requester);
    let invocation = rig.invocation(k, requester);
    let completion = adapter.execute(&invocation, &input)?;
    let token = adapter.prepare(k, &completion.effect)?;
    adapter.commit(k, &token)?;
    Ok(completion)
}

// HP-02 through Services: a store is assessed from its header alone, staged with a zero-time template, finalized at the decision time
// and committed at exactly that time; the result names the placement, acceptance 1000, expiry 1010 and the price 17.
#[test]
fn a_store_is_staged_finalized_and_committed() {
    let rig = rig();
    let adapter = rig.adapter();
    let publication = public(&rig.publisher, 1, Kind::Page, 1, b"abc");
    let me = rig.me();
    let k = key(1, me);
    // Header assessment sees a body of declared length without receiving it.
    let value = codec::request_value(&Request::Store(StoreRequest {
        publication: publication.clone(),
        retention_seconds: 10,
    }));
    let Value::Object(fields) = value.clone() else {
        unreachable!()
    };
    let described = Input::Object(
        fields
            .iter()
            .map(|(name, v)| match (name.as_str(), v) {
                ("content", Value::Bytes(bytes)) => (
                    name.clone(),
                    Input::Body {
                        length: bytes.len() as u64,
                        text: false,
                    },
                ),
                // The endpoint describes an array member as an unresolved array, never as a complete value.
                ("audience", Value::Array(items)) => (
                    name.clone(),
                    Input::Array(
                        items
                            .iter()
                            .map(|item| Input::Complete(item.clone()))
                            .collect(),
                    ),
                ),
                _ => (name.clone(), Input::Complete(v.clone())),
            })
            .collect(),
    );
    let assessment = adapter
        .assess_metadata(rig.peer(k, me), &offering(), &described, BUDGET)
        .unwrap();
    let Value::Object(metadata) = &assessment.metadata else {
        unreachable!()
    };
    assert!(
        metadata
            .iter()
            .any(|(n, v)| n == "contentSize" && *v == Value::Uint64(3))
    );
    assert!(!metadata.iter().any(|(n, _)| n == "content"));
    // Staging, preparation and the finalization protocol.
    let invocation = rig.invocation(k, me);
    assert!(adapter.requires_finalization(&invocation, &value));
    let template = adapter.execute(&invocation, &value).unwrap();
    assert_eq!(template.price, 17);
    assert_eq!(
        adapter.execute(&invocation, &value).unwrap().output,
        template.output
    );
    assert_eq!(rig.participant.provider().store().records().len(), 0);
    adapter
        .validate_template(&invocation, &value, &template)
        .unwrap();
    let token = adapter.prepare(k, &template.effect).unwrap();
    assert!(matches!(adapter.status(k), Ok(Status::Prepared(_))));
    rig.clock.advance(3);
    let finalized = adapter.finalize(k, &token, &template).unwrap();
    assert_eq!(
        adapter.validate_local_result(&invocation, &value, &finalized),
        Ok(true)
    );
    let Ok(Response::Store(output, price)) = codec::parse_response(&finalized.output) else {
        panic!("store response")
    };
    assert_eq!(
        (output.accepted_at, output.expires_at, price),
        (1003, 1013, 17)
    );
    adapter
        .commit_finalized(k, &token, &template, &finalized)
        .unwrap();
    assert!(matches!(adapter.status(k), Ok(Status::Committed)));
    let record = rig
        .participant
        .provider()
        .store()
        .record(output.placement_id)
        .unwrap();
    assert_eq!(
        (record.accepted_at, record.expires_at, record.payments),
        (1003, 1013, 17)
    );
    // A finalized result that disagrees with the commit (another time) is refused.
    let mut forged = finalized.clone();
    forged.output = codec::response_value(&Response::Store(
        crate::modules::hosted_publications::operation::StoreOutput {
            accepted_at: 1004,
            ..output
        },
        17,
    ));
    assert_eq!(
        adapter.commit_finalized(k, &token, &template, &forged),
        Err(Error::Conflict)
    );
}

// A staged operation survives a restart: a fresh adapter over the same participant rebuilds the same completion from the journal, and an
// unknown key is `Unknown`, never guessed.
#[test]
fn staged_results_are_rebuilt_not_repeated() {
    let rig = rig();
    let adapter = rig.adapter();
    let publication = public(&rig.publisher, 1, Kind::Page, 1, b"abc");
    let value = codec::request_value(&Request::Store(StoreRequest {
        publication,
        retention_seconds: 10,
    }));
    let k = key(1, rig.me());
    let template = adapter
        .execute(&rig.invocation(k, rig.me()), &value)
        .unwrap();
    let again = rig.adapter().completion(k).unwrap();
    assert_eq!(again.output, template.output);
    assert_eq!(again.price, template.price);
    assert_eq!(again.effect.bytes(), template.effect.bytes());
    assert_eq!(
        rig.adapter().completion(key(9, rig.me())).err(),
        Some(Error::Unknown)
    );
    // A wrong effect blob or token is refused.
    assert_eq!(
        adapter
            .prepare(k, &Blob::new(b"other".to_vec(), 16).unwrap())
            .err(),
        Some(Error::Conflict)
    );
    assert_eq!(
        adapter.prepare(key(9, rig.me()), &template.effect).err(),
        Some(Error::Denied)
    );
    let wrong = Blob::new(vec![1; 32], 32).unwrap();
    assert!(adapter.finalize(k, &wrong, &template).is_err());
    adapter.abort(k, None).unwrap();
    assert!(matches!(adapter.status(k), Ok(Status::Aborted)));
    assert!(rig.participant.provider().store().records().is_empty());
}

// Assessment denies requests addressed to another identity, from another endpoint than the requester, or against a wanted offering,
// and refuses a participant that has not recovered.
#[test]
fn assessment_is_bound_to_the_host_and_the_requester() {
    let rig = rig();
    let adapter = rig.adapter();
    let me = rig.me();
    let value = codec::request_value(&Request::List(ListRequest {
        publisher_id: me,
        kind: Kind::Page,
        before: None,
        limit: 1,
    }));
    let k = key(1, me);
    let good = Peer {
        local: HOST,
        remote: me,
        key: k,
    };
    assert!(adapter.assess(good, &offering(), &value).is_ok());
    assert_eq!(
        adapter
            .assess(
                Peer {
                    local: IdentityId([9; 32]),
                    ..good
                },
                &offering(),
                &value
            )
            .err(),
        Some(Error::Denied)
    );
    assert_eq!(
        adapter
            .assess(
                Peer {
                    remote: IdentityId([9; 32]),
                    ..good
                },
                &offering(),
                &value
            )
            .err(),
        Some(Error::Denied)
    );
    let mut wanted = offering();
    wanted.direction = crate::modules::board::offering::Direction::Wanted;
    assert_eq!(
        adapter.assess(good, &wanted, &value).err(),
        Some(Error::Denied)
    );
    let unrecovered = Arc::new(Participant::new(
        rig.participant.provider().clone(),
        Journal::volatile(4),
        Arc::new(SequenceEntropy::new(1)),
    ));
    assert_eq!(
        ServiceProvider::new(unrecovered, BUDGET, 100).err(),
        Some(Error::Unavailable)
    );
    assert_eq!(
        ServiceProvider::new(rig.participant.clone(), Budget { depth: 0, ..BUDGET }, 100).err(),
        Some(Error::Invalid)
    );
    // A list limit above the offering and a resource kind are invalid at assessment.
    for (kind, limit) in [(Kind::Page, 3), (Kind::Resource, 1)] {
        let bad = codec::request_value(&Request::List(ListRequest {
            publisher_id: me,
            kind,
            before: None,
            limit,
        }));
        assert_eq!(
            adapter.assess(good, &offering(), &bad).err(),
            Some(Error::Invalid),
            "{kind:?}"
        );
    }
}

// Get and list are read-only: assessment states the exact content size as the get factor, the staged result is read live, and a
// publication the reader may not read is concealed; the result carries the price.
#[test]
fn reads_state_their_size_and_return_the_publication() {
    let rig = rig();
    let me = rig.me();
    let (publication, _, _) = stored(&rig, 1, Kind::Page, 1, b"abc");
    let reader = IdentityId([8; 32]);
    let get = GetRequest {
        publisher_id: me,
        publication_id: PublicationId([1; 32]),
        maximum_bytes: 100,
    };
    let adapter = rig.adapter();
    let value = codec::request_value(&Request::Get(get));
    let assessment = adapter
        .assess(rig.peer(key(2, reader), reader), &offering(), &value)
        .unwrap();
    assert_eq!(
        assessment.factors,
        Value::Object(vec![("contentSize".into(), Value::Uint64(3))])
    );
    let done = run(&rig, 2, reader, &Request::Get(get)).unwrap();
    assert_eq!(done.price, 2);
    let Ok(Response::Get(output, 2)) = codec::parse_response(&done.output) else {
        panic!("get response")
    };
    assert_eq!(output.publication, publication);
    // A bound below the content size is a limit; an unknown publication is concealed as a denial.
    let small = codec::request_value(&Request::Get(GetRequest {
        maximum_bytes: 2,
        ..get
    }));
    assert_eq!(
        adapter
            .assess(rig.peer(key(3, reader), reader), &offering(), &small)
            .err(),
        Some(Error::Limit)
    );
    let unknown = codec::request_value(&Request::Get(GetRequest {
        publication_id: PublicationId([9; 32]),
        ..get
    }));
    assert_eq!(
        adapter
            .assess(rig.peer(key(3, reader), reader), &offering(), &unknown)
            .err(),
        Some(Error::Denied)
    );
    let listed = run(
        &rig,
        4,
        reader,
        &Request::List(ListRequest {
            publisher_id: me,
            kind: Kind::Page,
            before: None,
            limit: 2,
        }),
    )
    .unwrap();
    let Ok(Response::List(page, 1)) = codec::parse_response(&listed.output) else {
        panic!("list response")
    };
    assert_eq!(page.entries.len(), 1);
}

// Renewal and removal carry their quoted factors, stage and commit as prepared, issue the release credits once, and a recorded
// removal may be retried by its publisher only.
#[test]
fn renewal_and_removal_commit_and_retry() {
    let rig = rig();
    let me = rig.me();
    let (_, finalized, _) = stored(&rig, 1, Kind::Page, 1, b"abc");
    let Ok(Response::Store(stored_output, _)) = codec::parse_response(&finalized.output) else {
        panic!("store")
    };
    let placement = stored_output.placement_id;
    let adapter = rig.adapter();
    let renew = Request::Renew(RenewRequest {
        placement_id: placement,
        expected_expires_at: 1010,
        additional_seconds: 10,
    });
    let assessment = adapter
        .assess(
            rig.peer(key(5, me), me),
            &offering(),
            &codec::request_value(&renew),
        )
        .unwrap();
    assert_eq!(
        assessment.factors,
        Value::Object(vec![("contentSize".into(), Value::Uint64(3))])
    );
    let renewed = run(&rig, 5, me, &renew).unwrap();
    assert_eq!(renewed.price, 15);
    assert_eq!(
        rig.participant
            .provider()
            .store()
            .record(placement)
            .unwrap()
            .expires_at,
        1020
    );
    let remove = Request::Remove(RemoveRequest {
        placement_id: placement,
        removal_id: RemovalId([4; 32]),
        expected_expires_at: 1020,
        minimum_release_credits: 1,
    });
    let value = codec::request_value(&remove);
    let assessment = adapter
        .assess(rig.peer(key(6, me), me), &offering(), &value)
        .unwrap();
    // 30 percent of the 32 credits paid (store 17 plus renewal 15) is 9.
    assert_eq!(
        assessment.factors,
        Value::Object(vec![("releaseCredits".into(), Value::Uint64(9))])
    );
    let removed = run(&rig, 6, me, &remove).unwrap();
    assert_eq!(removed.price, 2);
    assert_eq!(rig.issuer.issued().len(), 1);
    // The retry assesses as a replay with the recorded credits, and only the publisher may use the cached answer.
    let original = rig.invocation(key(6, me), me);
    assert_eq!(
        adapter.allow_cached_retry(&original, rig.peer(key(7, me), me), &value),
        Ok(true)
    );
    let other = IdentityId([8; 32]);
    assert_eq!(
        adapter.allow_cached_retry(&original, rig.peer(key(7, other), other), &value),
        Ok(false)
    );
    let list = codec::request_value(&Request::List(ListRequest {
        publisher_id: me,
        kind: Kind::Page,
        before: None,
        limit: 1,
    }));
    assert_eq!(
        adapter.allow_cached_retry(&original, rig.peer(key(7, me), me), &list),
        Ok(false)
    );
    // A removal that would conflict with a prepared effect is refused at staging.
    assert_eq!(
        adapter
            .execute(&rig.invocation(key(8, me), me), &value)
            .err(),
        Some(Error::Conflict)
    );
}

// Evidence is one authorized content byte for the publisher of a pending placement; strangers and bad offsets are refused.
#[test]
fn evidence_serves_one_byte_to_the_publisher() {
    let rig = rig();
    let me = rig.me();
    let (_, finalized, _) = stored(&rig, 1, Kind::Page, 1, b"abc");
    let Ok(Response::Store(output, _)) = codec::parse_response(&finalized.output) else {
        panic!("store")
    };
    let adapter = rig.adapter();
    let check = |who: IdentityId, offset: u64| {
        let invocation = CheckInvocation {
            key: key(1, who),
            requester: who,
            provider: HOST,
            issuer: HOST,
            offering: offering(),
            definition: Catalogue::new(Default::default())
                .unwrap()
                .definition(&offering())
                .unwrap(),
            quote: Blob::new(b"quote".to_vec(), 16).unwrap(),
            price: 3,
            response_milliseconds: 5000,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(5),
        };
        adapter.evidence(
            &invocation,
            &codec::check_input_value(&CheckRequest {
                placement_id: output.placement_id,
                offset,
            }),
        )
    };
    let served = check(me, 1).unwrap();
    assert_eq!(codec::parse_check_output(&served).unwrap().value, b'b');
    assert_eq!(check(me, 3).err(), Some(Error::Invalid));
    assert_eq!(check(IdentityId([8; 32]), 1).err(), Some(Error::Denied));
}
