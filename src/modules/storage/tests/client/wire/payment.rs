//! Requester payment-mode integration uses real coordination and independent issuer accounting; remote Storage output remains controlled.
use super::*;
use ledger::payment::{Payment as State, Receipt};

// Finish one complete original response, then repeat commitment to expose accidental double charging or result execution.
pub(super) fn complete<
    L: ledger::persistence::Persistence,
    S: services::persistence::Persistence,
>(
    endpoint: &Endpoint<L, S>,
    input: Value,
    output: &Value,
    price: u64,
) {
    let outgoing = endpoint.reserve(input, price, json!({}));
    let (grant, mut receiver) = result::receiver(endpoint, &outgoing);
    result::deliver(endpoint, &grant, &mut receiver, output, price, true);
    let coordinate = endpoint
        .services
        .grant(
            endpoint.association.local,
            services::authority::Action::Coordinate,
            16_777_216,
        )
        .unwrap();
    receiver
        .finish(
            &pc(&grant),
            endpoint.association,
            true,
            &sc(&coordinate),
            &endpoint.runtime,
        )
        .unwrap();
    endpoint
        .runtime
        .prepare(&sc(&coordinate), endpoint.key())
        .unwrap();
    endpoint
        .runtime
        .decide(
            &sc(&coordinate),
            endpoint.key(),
            services::runtime::Decision::Commit,
        )
        .unwrap();
    endpoint
        .runtime
        .commit(&sc(&coordinate), endpoint.key())
        .unwrap();
    endpoint
        .runtime
        .commit(&sc(&coordinate), endpoint.key())
        .unwrap();
    assert_eq!(endpoint.phase(), Some(services::runtime::Phase::Committed));
}

// Requester-issued credits consume finite issuance and create one provider balance, without fabricating self-owned credits.
#[test]
fn requester_currency_uses_finite_issuance_once() {
    let f = fixture();
    let (inv, input, output) = store_fixture(false);
    let endpoint = endpoint_with_payment(f.client, &inv, inv.requester, Payment::default());
    assert_eq!(endpoint.available(), CREDIT);
    complete(&endpoint, input, &output, 17);
    assert_eq!(endpoint.available(), CREDIT - 17);
    let read = endpoint
        .ledger_authority
        .grant(inv.requester, ledger::authority::Action::Read, 65536)
        .unwrap();
    let view = endpoint.ledger.view(&lc(&read), inv.provider).unwrap();
    assert_eq!(view.issued, 17);
    assert_eq!(view.owned, 0);
    assert_eq!(view.trust.millionths(), 0);
    assert!(endpoint.ledger.view(&lc(&read), inv.requester).is_err());
    let observe = endpoint
        .services
        .grant(inv.requester, services::authority::Action::Observe, 65536)
        .unwrap();
    assert_eq!(
        endpoint.runtime.payment(&sc(&observe), inv.key).unwrap(),
        State::Settled
    );
}

// Generate real owner-signed issuer settlement with separate authority and accounting, including same-transfer idempotence.
fn issuer_result(
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
        ledger::persistence::Memory::default(),
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
    let quotes = ledger::fees::Quotes::new(order.issuer, authority.clone(), 4).unwrap();
    let peer = ledger::fees::Peer {
        owner: order.owner,
        connection: [20; 32],
        stream: 1,
    };
    let quote = quotes
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
    let accepted = quotes.accept(&lc(&control), &quote, peer, fee, 2).unwrap();
    let result = issuer.commit_transfer(&lc(&control), &accepted).unwrap();
    assert_eq!(
        issuer.commit_transfer(&lc(&control), &accepted).unwrap(),
        result
    );
    let read = authority
        .grant(order.issuer, ledger::authority::Action::Read, 65536)
        .unwrap();
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

// Explicit local Evidence authority models independently authenticated issuer provenance; forwarded peer fields cannot construct receipts.
pub(super) fn receipt<
    L: ledger::persistence::Persistence,
    S: services::persistence::Persistence,
>(
    endpoint: &Endpoint<L, S>,
    result: ledger::transfer_state::TransferResult,
) -> Receipt {
    let grant = endpoint
        .ledger_authority
        .grant(
            endpoint.association.local,
            ledger::authority::Action::Evidence,
            65536,
        )
        .unwrap();
    Receipt::authenticated(
        &lc(&grant),
        endpoint.ledger_authority.clone(),
        endpoint.association.local,
        endpoint.selection.roles().2,
        result,
    )
    .unwrap()
}

// External-currency service completion remains distinct from uncertain/refused payment until exact original issuer evidence settles once.
#[test]
fn third_party_currency_requires_exact_issuer_settlement_after_work() {
    use crate::modules::cryptography::{
        credential,
        primitives::{keys, signature},
        test_support::ScriptedEntropy,
    };
    use std::collections::VecDeque;
    let mut entropy = ScriptedEntropy(VecDeque::from([Ok(vec![13; 32])]));
    let keys = keys::signing_key(&mut entropy).unwrap();
    let f = fixture();
    let (mut inv, input, output) = store_fixture(false);
    inv.requester = IdentityId(credential::identity_id(&keys.public_key).unwrap());
    inv.key.requester = inv.requester;
    let issuer = IdentityId([9; 32]);
    let endpoint = endpoint_with_payment(
        f.client,
        &inv,
        issuer,
        Payment {
            transfer: Some([14; 16]),
            fee: Some(2),
        },
    );
    // An accepted service ceiling excludes no fee: total consent must cover both parts before a reservation is possible.
    let too_small = endpoint.request(input.clone(), 18);
    assert!(
        endpoint
            .candidate(too_small, &endpoint.quote(17, json!({})))
            .is_err()
    );
    assert_eq!(endpoint.available(), CREDIT);
    complete(&endpoint, input, &output, 17);
    assert_eq!(endpoint.available(), CREDIT - 19);
    let coordinate = endpoint
        .services
        .grant(
            inv.requester,
            services::authority::Action::Coordinate,
            65536,
        )
        .unwrap();
    let observe = endpoint
        .services
        .grant(inv.requester, services::authority::Action::Observe, 65536)
        .unwrap();
    let pending = endpoint
        .runtime
        .payment_request(&sc(&coordinate), inv.key)
        .unwrap();
    assert!(pending.submit);
    assert_eq!(pending.order.issuer, issuer);
    assert_eq!(pending.order.owner, inv.requester);
    assert_eq!(pending.order.new_owner, inv.provider);
    assert_eq!(pending.order.transfer_id, [14; 16]);
    assert_eq!(pending.order.amount, 19);
    assert_eq!(pending.fee, 2);
    assert_eq!(
        endpoint.runtime.payment(&sc(&observe), inv.key).unwrap(),
        State::Pending
    );
    for (unknown, expected) in [(true, State::Unknown), (false, State::Unsettled)] {
        endpoint
            .runtime
            .payment_problem(&sc(&coordinate), inv.key, unknown)
            .unwrap();
        assert_eq!(
            endpoint.runtime.payment(&sc(&observe), inv.key).unwrap(),
            expected
        );
        assert_eq!(endpoint.available(), CREDIT - 19);
        assert_eq!(endpoint.phase(), Some(services::runtime::Phase::Committed));
        assert_eq!(
            endpoint
                .runtime
                .payment_request(&sc(&coordinate), inv.key)
                .unwrap(),
            pending
        );
    }
    let signature =
        signature::sign(&keys.private_key, &pending.order.encode().unwrap(), 121).unwrap();
    let result = issuer_result(&pending.order, pending.fee, keys.public_key, signature);
    // Plausible but different transfer, recipient, gross or fee evidence cannot consume the original unresolved hold.
    for case in 0..4 {
        let mut wrong = result.clone();
        match case {
            0 => wrong.order.transfer_id = [15; 16],
            1 => wrong.order.new_owner = IdentityId([16; 32]),
            2 => wrong.order.amount += 1,
            _ => wrong.fee += 1,
        }
        assert!(
            endpoint
                .runtime
                .confirm_payment(&sc(&coordinate), inv.key, &receipt(&endpoint, wrong))
                .is_err()
        );
        assert_eq!(endpoint.available(), CREDIT - 19);
        assert_eq!(
            endpoint.runtime.payment(&sc(&observe), inv.key).unwrap(),
            State::Unsettled
        );
    }
    let receipt = receipt(&endpoint, result);
    endpoint
        .runtime
        .confirm_payment(&sc(&coordinate), inv.key, &receipt)
        .unwrap();
    endpoint
        .runtime
        .confirm_payment(&sc(&coordinate), inv.key, &receipt)
        .unwrap();
    assert_eq!(
        endpoint.runtime.payment(&sc(&observe), inv.key).unwrap(),
        State::Settled
    );
    assert_eq!(endpoint.available(), CREDIT - 19);
    assert!(
        endpoint
            .runtime
            .payment_request(&sc(&coordinate), inv.key)
            .is_err()
    );
    assert!(
        endpoint
            .runtime
            .payment_problem(&sc(&coordinate), inv.key, true)
            .is_err()
    );
    let read = endpoint
        .ledger_authority
        .grant(inv.requester, ledger::authority::Action::Read, 65536)
        .unwrap();
    let view = endpoint.ledger.view(&lc(&read), issuer).unwrap();
    assert_eq!(view.owned, CREDIT - 19);
    assert_eq!(view.issued, 0);
    assert_eq!(view.trust.millionths(), 0);
    // Payment confirmation neither replaces nor revalidates the already retained Storage result.
    let retained = endpoint
        .runtime
        .result(&sc(&observe), inv.key)
        .unwrap()
        .output;
    let (Value::Object(actual), Value::Object(expected)) = (retained, output) else {
        panic!("Storage result");
    };
    assert_eq!(
        actual.into_iter().collect::<BTreeMap<_, _>>(),
        expected.into_iter().collect::<BTreeMap<_, _>>()
    );
}
