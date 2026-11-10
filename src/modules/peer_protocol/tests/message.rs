//! Required field and identifier rejection for every specified wire control message.
use super::{Error, base64, message::Message};
use serde_json::{Value, json};
// Every known message validates its independent field shape; deleting or nulling a required field always fails.
#[test]
fn all_message_shapes() {
    let identity =
        crate::modules::identity::text::format(crate::modules::identity::api::IdentityId([0; 32]));
    let mut samples = vec![];
    {
        let v = json!({"type":"board.get"});
        samples.push(v);
    }
    {
        let mut v = json!({"type":"service.get"});
        v["offeringId"] = json!("name");
        samples.push(v);
    }
    {
        let mut v = json!({"type":"board"});
        v["board"] = json!({});
        samples.push(v);
    }
    {
        let mut v = json!({"type":"definition"});
        v["definition"] = json!({});
        samples.push(v);
    }
    {
        let mut v = json!({"type":"offer"});
        v["offeringId"] = json!("name");
        v["service"] = json!("name");
        v["serviceVersion"] = json!(1);
        samples.push(v);
    }
    {
        let mut v = json!({"type":"propose"});
        v["offeringId"] = json!("name");
        v["publisherId"] = json!(identity.clone());
        v["service"] = json!("name");
        v["serviceVersion"] = json!(1);
        v["mode"] = json!("invoke");
        v["input"] = json!(false);
        v["creditIssuer"] = json!(identity.clone());
        v["maximumPrice"] = json!("7");
        samples.push(v);
    }
    {
        let mut v = json!({"type":"quote"});
        v["quoteId"] = json!(base64::encode(&[0; 32]));
        v["offering"] = json!({});
        v["creditIssuer"] = json!(identity.clone());
        v["priceMode"] = json!("fixed");
        v["maximumPrice"] = json!("7");
        v["pricingInputs"] = json!({});
        samples.push(v);
    }
    {
        let mut v = json!({"type":"accept"});
        v["quoteId"] = json!(base64::encode(&[0; 32]));
        v["quoteHash"] = json!(base64::encode(&[0; 32]));
        samples.push(v);
    }
    {
        let v = json!({"type":"input.end"});
        samples.push(v);
    }
    {
        let mut v = json!({"type":"result"});
        v["output"] = json!(false);
        v["finalPrice"] = json!("7");
        samples.push(v);
    }
    {
        let v = json!({"type":"result.end"});
        samples.push(v);
    }
    {
        let mut v = json!({"type":"failure"});
        v["code"] = json!("name");
        samples.push(v);
    }
    {
        let v = json!({"type":"ledger.balance"});
        samples.push(v);
    }
    {
        let mut v = json!({"type":"ledger.balance.result"});
        v["balance"] = json!("7");
        samples.push(v);
    }
    {
        let mut v = json!({"type":"ledger.transfer"});
        v["order"] = json!(base64::encode(&[0; 121]));
        v["ownerPublicKey"] = json!(base64::encode(&[0; 32]));
        v["signature"] = json!(base64::encode(&[0; 64]));
        v["maximumFee"] = json!("7");
        samples.push(v);
    }
    {
        let mut v = json!({"type":"ledger.transfer.quote"});
        v["transferId"] = json!(base64::encode(&[0; 16]));
        v["fee"] = json!("7");
        samples.push(v);
    }
    {
        let mut v = json!({"type":"ledger.transfer.accept"});
        v["transferId"] = json!(base64::encode(&[0; 16]));
        v["fee"] = json!("7");
        samples.push(v);
    }
    {
        let mut v = json!({"type":"ledger.transfer.get"});
        v["ownerId"] = json!(identity.clone());
        v["transferId"] = json!(base64::encode(&[0; 16]));
        samples.push(v);
    }
    {
        let mut v = json!({"type":"ledger.transfer.result"});
        v["ownerId"] = json!(identity.clone());
        v["newOwnerId"] = json!(identity.clone());
        v["transferId"] = json!(base64::encode(&[0; 16]));
        v["amount"] = json!("7");
        v["fee"] = json!("7");
        v["receivedAmount"] = json!("7");
        samples.push(v);
    }
    {
        let mut v = json!({"type":"ledger.claim.redeem"});
        v["envelope"] = json!("AA==");
        samples.push(v);
    }
    {
        let mut v = json!({"type":"ledger.claim.result"});
        v["claimId"] = json!(base64::encode(&[0; 32]));
        v["amount"] = json!("7");
        samples.push(v);
    }
    for v in samples {
        let encoded = v.to_string();
        let message = Message::parse(encoded.as_bytes(), 65536).unwrap();
        assert_eq!(message.bytes(), encoded.as_bytes());
        assert_eq!(message.value(), &v);
        let _ = message.kind();
        for key in v.as_object().unwrap().keys().filter(|k| *k != "type") {
            let mut bad = v.clone();
            bad[key] = Value::Null;
            assert!(Message::parse(bad.to_string().as_bytes(), 65536).is_err());
            bad.as_object_mut().unwrap().remove(key);
            assert!(Message::parse(bad.to_string().as_bytes(), 65536).is_err());
        }
    }
    assert!(matches!(
        Message::parse(br#"{"type":"made.up"}"#, 32),
        Err(Error::Unsupported)
    ));
    for value in ["01", "+1", "1e2", "18446744073709551616"] {
        let raw = json!({"type":"ledger.balance.result","balance":value}).to_string();
        assert!(Message::parse(raw.as_bytes(), 32).is_err());
    }
    let raw =
        json!({"type":"failure","code":"future_failure","message":"x".repeat(1025)}).to_string();
    assert!(Message::parse(raw.as_bytes(), 32).is_err());
    let raw=json!({"type":"failure","code":"future_failure","message":"x".repeat(1024),"optional":null}).to_string();
    assert!(Message::parse(raw.as_bytes(), 32).is_ok());
}
// Canonical standard alphabet and unused bits are checked independently of successful local round trips.
#[test]
fn canonical_base64() {
    for (raw, text) in [
        (b"".as_slice(), ""),
        (b"a", "YQ=="),
        (b"ab", "YWI="),
        (b"abc", "YWJj"),
        (&[0, 255], "AP8="),
    ] {
        assert_eq!(base64::encode(raw), text);
        assert_eq!(base64::decode(text, 4096).unwrap(), raw);
    }
    for text in [
        "YQ", "YR==", "Y WJj", "YWJ=", "=AAA", "AA=A", "A===", "____", "AA==AAAA",
    ] {
        assert_eq!(base64::decode(text, 4096), Err(Error::Invalid));
    }
    assert_eq!(base64::decode("YWJj", 2), Err(Error::Limit));
}
