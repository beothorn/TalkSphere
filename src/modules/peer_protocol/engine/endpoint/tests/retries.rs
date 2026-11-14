//! Explicit family-approved cached retries reuse original work/payment keys; optional Messaging signatures/resources remain test substitutes.
use super::*;
use peer::test_support::{messaging::Messaging, world::World};
// Lost replies after provider commitment are recovered by a deliberate original-key retry without current price or provider availability.
#[test]
fn explicit_cached_retry_completes_original_financial_operation_once() {
    for remove in [false, true] {
        let pair = Pair::from_worlds(
            World::build(false, Arc::new(Messaging::new())),
            World::build(true, Arc::new(Messaging::new())),
        );
        let input = Messaging::input(remove);
        let (issued, request) = pair.propose(1, input.clone(), 7);
        let original = issued.association();
        let outgoing = pair.reserve(&issued, request).unwrap();
        let key = outgoing.invocation().key;
        let incoming = pair
            .accept(
                issued,
                original,
                &Message::parse(outgoing.accept(), 65536).unwrap(),
            )
            .unwrap();
        pair.commit_provider(incoming, &input);
        drop(outgoing);
        let p = &pair.provider;
        let r = &pair.requester;
        let register = p
            .services
            .grant(p.local, services::authority::Action::Register, 16_777_216)
            .unwrap();
        p.registry
            .drain(&service_context(&register), "cr2se.messaging", 1)
            .unwrap();
        let provider = endpoint(&pair);
        let requester = endpoint_world(r);
        let prices = p.family.prices.load(Ordering::SeqCst);
        for stream in [3, 5] {
            let reply = call(&requester, &provider, stream, requests::Plan::Retry(key));
            let Ok(requests::Reply::Work(completed)) = reply.result else {
                panic!("original committed cached result")
            };
            assert_eq!(completed, key);
            assert_eq!(pair.owned_available(), 93);
            assert_eq!(p.family.executions(), 1);
            assert_eq!(p.family.prices.load(Ordering::SeqCst), prices);
        }
    }
}
