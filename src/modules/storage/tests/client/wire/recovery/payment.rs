//! Native requester currency and independent issuer recovery, using controlled Storage results and synthetic authenticated issuer provenance.
use super::super::payment::{complete, receipt};
use super::*;
use crate::modules::cryptography::{
    credential,
    primitives::{keys, signature},
    test_support::ScriptedEntropy,
};
use ledger::payment::Payment as State;
use std::collections::VecDeque;

// Obtain a fresh Observe grant instead of allowing a previous process's authority to read the newly opened coordinator.
fn state(endpoint: &Native) -> State {
    let grant = endpoint
        .services
        .grant(
            endpoint.association.local,
            services::authority::Action::Observe,
            65536,
        )
        .unwrap();
    endpoint
        .runtime
        .payment(&sc(&grant), endpoint.key())
        .unwrap()
}
// Read the original issuer-specific account view independently of unresolved holds and available spending capacity.
fn view(endpoint: &Native, peer: IdentityId) -> ledger::accounting::View {
    let grant = endpoint
        .ledger_authority
        .grant(
            endpoint.association.local,
            ledger::authority::Action::Read,
            65536,
        )
        .unwrap();
    endpoint.ledger.view(&lc(&grant), peer).unwrap()
}

// Requester-issued work consumes finite issuance and creates exactly one provider balance after recovery at every completion boundary.
#[test]
fn native_requester_issuance_is_not_recreated_after_restart() {
    let (inv, input, output) = store_fixture(false);
    for boundary in 0..4 {
        let directory = Directory::new();
        let endpoint =
            open_with_payment(&directory.0, &inv, inv.requester, Payment::default(), true);
        let outgoing = endpoint.reserve(input.clone(), 17, json!({}));
        stage(&endpoint, &outgoing, &output, 17);
        let grant = coordinate(&endpoint);
        if boundary >= 1 {
            endpoint.runtime.prepare(&sc(&grant), inv.key).unwrap();
        }
        if boundary >= 2 {
            endpoint
                .runtime
                .decide(&sc(&grant), inv.key, Decision::Commit)
                .unwrap();
        }
        if boundary >= 3 {
            endpoint.runtime.commit(&sc(&grant), inv.key).unwrap();
        }
        assert_eq!(endpoint.available(), CREDIT - 17);
        drop(outgoing);
        drop(endpoint);
        let endpoint =
            open_with_payment(&directory.0, &inv, inv.requester, Payment::default(), false);
        assert_eq!(endpoint.available(), CREDIT - 17);
        let grant = coordinate(&endpoint);
        assert_eq!(
            endpoint.runtime.resume(&sc(&grant), inv.key).unwrap(),
            Phase::Committed
        );
        assert_eq!(
            endpoint.runtime.resume(&sc(&grant), inv.key).unwrap(),
            Phase::Committed
        );
        same_output(completed(&endpoint).unwrap().output, &output);
        assert_eq!(view(&endpoint, inv.provider).issued, 17);
        assert_eq!(view(&endpoint, inv.provider).owned, 0);
        assert_eq!(state(&endpoint), State::Settled);
        drop(endpoint);
        let endpoint =
            open_with_payment(&directory.0, &inv, inv.requester, Payment::default(), false);
        assert_eq!(endpoint.available(), CREDIT - 17);
        assert_eq!(view(&endpoint, inv.provider).issued, 17);
        let read = endpoint
            .ledger_authority
            .grant(inv.requester, ledger::authority::Action::Read, 65536)
            .unwrap();
        assert!(endpoint.ledger.view(&lc(&read), inv.requester).is_err());
    }
}

// Perform a real signed transfer at an independently persisted issuer; deliberately discard its result before querying the reopened original record.
fn transfer_after_lost_reply(
    path: &Path,
    order: &ledger::transfer::Order,
    fee: u64,
    public: [u8; 32],
    signature: [u8; 64],
) -> ledger::transfer_state::TransferResult {
    let authority = Arc::new(ledger::authority::Authority::new());
    let admin = authority
        .grant(order.issuer, ledger::authority::Action::Administer, 65536)
        .unwrap();
    let issuer = ledger::state::Ledger::initialize(
        order.issuer,
        authority.clone(),
        ledger::linux::FileStore::open(&path.join("issuer"), Default::default(), true).unwrap(),
        Default::default(),
        &lc(&admin),
        CREDIT,
        CREDIT,
    )
    .unwrap();
    issuer.grant(&lc(&admin), order.owner, CREDIT).unwrap();
    let control = authority
        .grant(order.issuer, ledger::authority::Action::Control, 65536)
        .unwrap();
    let quotes = ledger::fees::Quotes::new(order.issuer, authority.clone(), 1).unwrap();
    let peer = ledger::fees::Peer {
        owner: order.owner,
        connection: [31; 32],
        stream: 1,
    };
    let quoted = quotes
        .quote(
            &lc(&control),
            peer,
            ledger::fees::Request {
                order: order.clone(),
                key: public,
                signature,
                maximum_fee: fee,
                fee,
                expires: 100,
            },
            1,
        )
        .unwrap();
    let accepted = quotes.accept(&lc(&control), &quoted, peer, fee, 2).unwrap();
    issuer.commit_transfer(&lc(&control), &accepted).unwrap();
    drop(issuer);
    // All issuer ledger state and grants are recreated; only its protected native record supplies the complete original transfer response.
    let authority = Arc::new(ledger::authority::Authority::new());
    let read = authority
        .grant(order.issuer, ledger::authority::Action::Read, 65536)
        .unwrap();
    let issuer = ledger::state::Ledger::open(
        order.issuer,
        authority.clone(),
        ledger::linux::FileStore::open(&path.join("issuer"), Default::default(), false).unwrap(),
        Default::default(),
        &lc(&read),
    )
    .unwrap();
    let control = authority
        .grant(order.issuer, ledger::authority::Action::Control, 65536)
        .unwrap();
    assert!(
        issuer
            .transfer_result(
                &lc(&control),
                IdentityId([32; 32]),
                order.owner,
                order.transfer_id
            )
            .is_err()
    );
    let result = issuer
        .transfer_result(&lc(&control), order.owner, order.owner, order.transfer_id)
        .unwrap()
        .unwrap();
    assert_eq!(result.order, *order);
    assert_eq!(result.fee, fee);
    assert_eq!(
        issuer
            .transfer_result(
                &lc(&control),
                order.new_owner,
                order.owner,
                order.transfer_id
            )
            .unwrap(),
        Some(result.clone())
    );
    assert_eq!(
        issuer.view(&lc(&read), order.owner).unwrap().issued,
        CREDIT - order.amount
    );
    assert_eq!(
        issuer.view(&lc(&read), order.new_owner).unwrap().issued,
        order.amount - fee
    );
    result
}

// Third-party payment remains independently pending/unknown/unsettled through native restart; only exact evidence from a recovered issuer settles once.
#[test]
fn native_third_party_settlement_survives_requester_and_issuer_restart() {
    let mut entropy = ScriptedEntropy(VecDeque::from([Ok(vec![30; 32])]));
    let keys = keys::signing_key(&mut entropy).unwrap();
    let (mut inv, input, output) = store_fixture(false);
    inv.requester = IdentityId(credential::identity_id(&keys.public_key).unwrap());
    inv.key.requester = inv.requester;
    let issuer = IdentityId([9; 32]);
    let payment = Payment {
        transfer: Some([33; 16]),
        fee: Some(2),
    };
    for expected in [State::Pending, State::Unknown, State::Unsettled] {
        let directory = Directory::new();
        let endpoint = open_with_payment(&directory.0, &inv, issuer, payment, true);
        complete(&endpoint, input.clone(), &output, 17);
        let original = completed(&endpoint).unwrap();
        let grant = coordinate(&endpoint);
        if expected != State::Pending {
            endpoint
                .runtime
                .payment_problem(&sc(&grant), inv.key, expected == State::Unknown)
                .unwrap();
        }
        let pending = endpoint
            .runtime
            .payment_request(&sc(&grant), inv.key)
            .unwrap();
        assert!(pending.submit);
        assert_eq!(pending.order.amount, 19);
        assert_eq!(pending.fee, 2);
        assert_eq!(endpoint.available(), CREDIT - 19);
        assert_eq!(view(&endpoint, issuer).owned, CREDIT);
        drop(endpoint);
        let endpoint = open_with_payment(&directory.0, &inv, issuer, payment, false);
        assert_eq!(endpoint.phase(), Some(Phase::Committed));
        assert_eq!(state(&endpoint), expected);
        assert_eq!(endpoint.available(), CREDIT - 19);
        assert_eq!(view(&endpoint, issuer).owned, CREDIT);
        let grant = coordinate(&endpoint);
        assert_eq!(
            endpoint
                .runtime
                .payment_request(&sc(&grant), inv.key)
                .unwrap(),
            pending
        );
        assert!(endpoint.runtime.outbound(&sc(&grant), inv.key).is_err());
        let signed =
            signature::sign(&keys.private_key, &pending.order.encode().unwrap(), 121).unwrap();
        let result = transfer_after_lost_reply(
            &directory.0,
            &pending.order,
            pending.fee,
            keys.public_key,
            signed,
        );
        let mut wrong = result.clone();
        wrong.order.transfer_id = [34; 16];
        assert!(
            endpoint
                .runtime
                .confirm_payment(&sc(&grant), inv.key, &receipt(&endpoint, wrong))
                .is_err()
        );
        assert_eq!(state(&endpoint), expected);
        let old_receipt = receipt(&endpoint, result.clone());
        endpoint
            .runtime
            .confirm_payment(&sc(&grant), inv.key, &old_receipt)
            .unwrap();
        endpoint
            .runtime
            .confirm_payment(&sc(&grant), inv.key, &old_receipt)
            .unwrap();
        assert_eq!(state(&endpoint), State::Settled);
        assert_eq!(view(&endpoint, issuer).owned, CREDIT - 19);
        drop(endpoint);
        let endpoint = open_with_payment(&directory.0, &inv, issuer, payment, false);
        let grant = coordinate(&endpoint);
        assert!(
            endpoint
                .runtime
                .confirm_payment(&sc(&grant), inv.key, &old_receipt)
                .is_err()
        );
        let fresh = receipt(&endpoint, result);
        endpoint
            .runtime
            .confirm_payment(&sc(&grant), inv.key, &fresh)
            .unwrap();
        assert_eq!(state(&endpoint), State::Settled);
        assert_eq!(endpoint.available(), CREDIT - 19);
        assert_eq!(view(&endpoint, issuer).owned, CREDIT - 19);
        assert_eq!(view(&endpoint, issuer).issued, 0);
        assert_eq!(view(&endpoint, issuer).trust.millionths(), 0);
        assert!(
            endpoint
                .runtime
                .payment_request(&sc(&grant), inv.key)
                .is_err()
        );
        assert!(
            endpoint
                .runtime
                .payment_problem(&sc(&grant), inv.key, true)
                .is_err()
        );
        let retained = completed(&endpoint).unwrap();
        same_output(retained.output, &output);
        assert_eq!(retained.result.bytes(), original.result.bytes());
        assert_eq!(retained.effect.bytes(), original.effect.bytes());
    }
}
