//! Directional half-close and terminal-order tests, independent of body/schema/accounting ports.
use super::{
    Error,
    exchange::{
        Direction::{Local, Remote},
        Exchange, Outcome,
    },
    message::Kind,
};
// Ordinary and wanted exchanges derive application roles from the offer/proposal flow rather than the opener.
#[test]
fn provided_and_wanted_lifecycle() {
    for wanted in [false, true] {
        let mut x = Exchange::new(Local);
        if wanted {
            x.control(Local, Kind::Offer).unwrap();
        }
        let (req, provider) = if wanted {
            (Remote, Local)
        } else {
            (Local, Remote)
        };
        assert_eq!(x.roles(), (req, provider));
        for (dir, kind) in [
            (req, Kind::Propose),
            (provider, Kind::Quote),
            (req, Kind::Accept),
        ] {
            x.control(dir, kind).unwrap();
        }
        assert!(x.accepted());
        x.body(req).unwrap();
        x.control(req, Kind::InputEnd).unwrap();
        x.end(req).unwrap();
        x.control(provider, Kind::Result).unwrap();
        x.body(provider).unwrap();
        x.control(provider, Kind::ResultEnd).unwrap();
        x.end(provider).unwrap();
        assert_eq!(x.close(true), Ok(Outcome::Complete));
        assert_eq!(x.close(true), Err(Error::Conflict));
    }
}
// Metadata and Ledger replies require their exact type and both ENDs; transfer owners cannot end before fee acceptance.
#[test]
fn metadata_and_ledger_order() {
    for (request, response) in [
        (Kind::BoardGet, Kind::Board),
        (Kind::ServiceGet, Kind::Definition),
        (Kind::LedgerBalance, Kind::LedgerBalanceResult),
        (Kind::LedgerTransferGet, Kind::LedgerTransferResult),
        (Kind::LedgerClaimRedeem, Kind::LedgerClaimResult),
    ] {
        let mut x = Exchange::new(Local);
        x.control(Local, request).unwrap();
        x.end(Local).unwrap();
        x.control(Remote, response).unwrap();
        x.end(Remote).unwrap();
        assert_eq!(x.close(true), Ok(Outcome::Complete));
    }
    let mut x = Exchange::new(Local);
    x.control(Local, Kind::LedgerTransfer).unwrap();
    assert_eq!(x.end(Local), Err(Error::Invalid));
    let mut x = Exchange::new(Local);
    x.control(Local, Kind::LedgerTransfer).unwrap();
    x.control(Remote, Kind::LedgerTransferQuote).unwrap();
    x.control(Local, Kind::LedgerTransferAccept).unwrap();
    x.end(Local).unwrap();
    x.control(Remote, Kind::LedgerTransferResult).unwrap();
    x.end(Remote).unwrap();
    assert_eq!(x.close(true), Ok(Outcome::Complete));
    let mut x = Exchange::new(Local);
    x.control(Local, Kind::LedgerTransfer).unwrap();
    x.control(Remote, Kind::LedgerTransferResult).unwrap();
    x.end(Remote).unwrap();
    x.end(Local).unwrap();
    assert_eq!(x.close(true), Ok(Outcome::Complete));
}
// Early/unrelated results, bodies and missing markers poison only the affected exchange; accepted errors retain uncertainty.
#[test]
fn illegal_order_and_interruption() {
    let mut x = Exchange::new(Local);
    assert_eq!(x.control(Remote, Kind::Result), Err(Error::Invalid));
    assert_eq!(x.control(Local, Kind::BoardGet), Err(Error::Conflict));
    let mut healthy = Exchange::new(Local);
    healthy.control(Local, Kind::BoardGet).unwrap();
    healthy.end(Local).unwrap();
    healthy.control(Remote, Kind::Board).unwrap();
    healthy.end(Remote).unwrap();
    assert_eq!(healthy.close(true), Ok(Outcome::Complete));
    for accepted in [false, true] {
        let mut x = Exchange::new(Local);
        x.control(Local, Kind::Propose).unwrap();
        x.control(Remote, Kind::Quote).unwrap();
        if accepted {
            x.control(Local, Kind::Accept).unwrap();
        }
        assert_eq!(
            x.close(false),
            Ok(if accepted {
                Outcome::Unknown
            } else {
                Outcome::Failure
            })
        );
    }
    let mut x = Exchange::new(Local);
    x.control(Local, Kind::Propose).unwrap();
    assert_eq!(x.body(Local), Err(Error::Invalid));
    let mut x = Exchange::new(Local);
    for (dir, kind) in [
        (Local, Kind::Propose),
        (Remote, Kind::Quote),
        (Local, Kind::Accept),
        (Local, Kind::InputEnd),
    ] {
        x.control(dir, kind).unwrap();
    }
    x.end(Local).unwrap();
    x.control(Remote, Kind::Result).unwrap();
    x.control(Remote, Kind::ResultEnd).unwrap();
    assert_eq!(x.control(Remote, Kind::Failure), Err(Error::Invalid));
    assert!(x.accepted());
    let mut x = Exchange::new(Local);
    x.control(Local, Kind::BoardGet).unwrap();
    assert_eq!(x.close(true), Err(Error::Invalid));
}
// Only complete explicit failure plus normal closure is a failure result; a lost terminal observation remains unknown after acceptance.
#[test]
fn explicit_failure_completion() {
    let mut x = Exchange::new(Local);
    for (dir, kind) in [
        (Local, Kind::Propose),
        (Remote, Kind::Quote),
        (Local, Kind::Accept),
        (Local, Kind::InputEnd),
    ] {
        x.control(dir, kind).unwrap();
    }
    x.end(Local).unwrap();
    x.control(Remote, Kind::Failure).unwrap();
    x.end(Remote).unwrap();
    assert_eq!(x.close(true), Ok(Outcome::Failure));
    let mut x = Exchange::new(Local);
    x.control(Local, Kind::Propose).unwrap();
    x.control(Remote, Kind::Quote).unwrap();
    x.control(Local, Kind::Accept).unwrap();
    x.control(Remote, Kind::Failure).unwrap();
    assert_eq!(x.close(false), Ok(Outcome::Unknown));
}
