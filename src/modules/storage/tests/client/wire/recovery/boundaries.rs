//! Native reopen boundaries test original-key requester progress and holds, without a provider, subprocess kill or power-loss claim.
use super::*;

// All four ordinary outputs survive independent owner reopen at staged, prepared, decided and acknowledged boundaries with one exact charge.
#[test]
fn native_requester_recovery_preserves_all_operations_and_exact_charges() {
    let (store, si, so) = store_fixture(false);
    let (renew, ri, ro, _) = renew_fixture();
    let (retrieve, gi, go, _) = retrieve();
    let (remove, di, do_, _) = remove_fixture();
    for (inv, input, output, price, factors) in [
        (store, si, so, 17, json!({})),
        (renew, ri, ro, 15, json!({"sizeBytes":"3"})),
        (retrieve, gi, go, 2, json!({})),
        (remove, di, do_, 2, json!({})),
    ] {
        for cut in 0..4 {
            let directory = Directory::new();
            let endpoint = open(&directory.0, &inv, true);
            let outgoing = endpoint.reserve(input.clone(), price, factors.clone());
            stage(&endpoint, &outgoing, &output, price);
            let grant = coordinate(&endpoint);
            if cut >= 1 {
                endpoint.runtime.prepare(&sc(&grant), inv.key).unwrap();
            }
            if cut >= 2 {
                endpoint
                    .runtime
                    .decide(&sc(&grant), inv.key, Decision::Commit)
                    .unwrap();
            }
            if cut >= 3 {
                endpoint.runtime.commit(&sc(&grant), inv.key).unwrap();
            }
            let expected_phase = [
                Phase::Staged,
                Phase::Prepared,
                Phase::Commit,
                Phase::Committed,
            ][cut];
            assert_eq!(endpoint.phase(), Some(expected_phase));
            assert_eq!(endpoint.available(), CREDIT - price);
            let old_result = (cut == 3).then(|| completed(&endpoint).unwrap());
            drop(outgoing);
            drop(endpoint);
            let endpoint = open(&directory.0, &inv, false);
            assert_eq!(endpoint.phase(), Some(expected_phase));
            assert_eq!(endpoint.available(), CREDIT - price);
            assert!(endpoint.runtime.resume(&sc(&grant), inv.key).is_err());
            let grant = coordinate(&endpoint);
            assert_eq!(
                endpoint.runtime.resume(&sc(&grant), inv.key).unwrap(),
                Phase::Committed
            );
            assert_eq!(
                endpoint.runtime.resume(&sc(&grant), inv.key).unwrap(),
                Phase::Committed
            );
            endpoint.runtime.commit(&sc(&grant), inv.key).unwrap();
            assert_eq!(endpoint.available(), CREDIT - price);
            let result = completed(&endpoint).unwrap();
            same_output(result.output.clone(), &output);
            assert_eq!(result.price, price);
            if let Some(old) = old_result {
                assert_eq!(result.result.bytes(), old.result.bytes());
                assert_eq!(result.effect.bytes(), old.effect.bytes());
            }
            let bytes = result.result;
            let effect = result.effect;
            drop(endpoint);
            let endpoint = open(&directory.0, &inv, false);
            let result = completed(&endpoint).unwrap();
            assert_eq!(result.result.bytes(), bytes.bytes());
            assert_eq!(result.effect.bytes(), effect.bytes());
            same_output(result.output, &output);
            assert_eq!(endpoint.available(), CREDIT - price);
        }
    }
}

// Missing or incomplete remote success stays unknown after native restart; resume/outbound cannot fabricate completion, release funds or resend work.
#[test]
fn unresolved_native_requester_holds_survive_without_resending() {
    let (inv, input, output) = store_fixture(false);
    for partial in [false, true] {
        let directory = Directory::new();
        let endpoint = open(&directory.0, &inv, true);
        let outgoing = endpoint.reserve(input.clone(), 17, json!({}));
        if partial {
            let (grant, mut receiver) = result::receiver(&endpoint, &outgoing);
            result::deliver(&endpoint, &grant, &mut receiver, &output, 17, false);
        }
        drop(outgoing);
        drop(endpoint);
        for _ in 0..2 {
            let endpoint = open(&directory.0, &inv, false);
            let grant = coordinate(&endpoint);
            assert_eq!(endpoint.phase(), Some(Phase::Executing));
            assert_eq!(
                endpoint.runtime.resume(&sc(&grant), inv.key),
                Err(Error::Unknown)
            );
            assert!(endpoint.runtime.outbound(&sc(&grant), inv.key).is_err());
            assert!(completed(&endpoint).is_err());
            assert_eq!(endpoint.available(), CREDIT - 17);
        }
    }
}

// A dispatched remote store may already be committed; restart cannot turn a local abort request into a refund, even with complete staged success.
#[test]
fn native_requester_recovery_cannot_abort_dispatched_work() {
    let (inv, input, output) = store_fixture(false);
    for boundary in 0..3 {
        let directory = Directory::new();
        let endpoint = open(&directory.0, &inv, true);
        let outgoing = endpoint.reserve(input.clone(), 17, json!({}));
        let grant = coordinate(&endpoint);
        if boundary >= 1 {
            stage(&endpoint, &outgoing, &output, 17);
        }
        if boundary >= 2 {
            endpoint.runtime.prepare(&sc(&grant), inv.key).unwrap();
        }
        let phase = [Phase::Executing, Phase::Staged, Phase::Prepared][boundary];
        assert_eq!(
            endpoint
                .runtime
                .decide(&sc(&grant), inv.key, Decision::Abort),
            Err(Error::Unknown)
        );
        drop(outgoing);
        drop(endpoint);
        let endpoint = open(&directory.0, &inv, false);
        let grant = coordinate(&endpoint);
        assert_eq!(endpoint.phase(), Some(phase));
        assert_eq!(
            endpoint
                .runtime
                .decide(&sc(&grant), inv.key, Decision::Abort),
            Err(Error::Unknown)
        );
        assert!(endpoint.runtime.abort(&sc(&grant), inv.key).is_err());
        assert_eq!(endpoint.phase(), Some(phase));
        assert!(completed(&endpoint).is_err());
        assert_eq!(endpoint.available(), CREDIT - 17);
    }
}
