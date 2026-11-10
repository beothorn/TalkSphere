//! Fixed inline byte thresholds and malicious wrapper spellings.
use super::{Error, inline};
use serde_json::json;
// Empty and4096-byte values are inline;4097 bytes require a body and cannot be smuggled through padded text.
#[test]
fn inline_boundaries() {
    assert_eq!(inline::encode(b"abc"), Ok(json!({"$base64":"YWJj"})));
    assert_eq!(inline::encode(b""), Ok(json!({"$base64":""})));
    for n in [0, 1, 4095, 4096] {
        let data = vec![255; n];
        assert_eq!(inline::decode(&inline::encode(&data).unwrap()), Ok(data));
    }
    assert_eq!(inline::encode(&vec![0; 4097]), Err(Error::Limit));
    let too_large = json!({"$base64":super::base64::encode(&vec![0;4097])});
    assert_eq!(inline::decode(&too_large), Err(Error::Limit));
    for value in [
        json!(null),
        json!({"$base64":"YQ==","extra":0}),
        json!({"$base64":null}),
        json!({"$base64":"YR=="}),
    ] {
        assert_eq!(inline::decode(&value), Err(Error::Invalid));
    }
}
