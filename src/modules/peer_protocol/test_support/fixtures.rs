//! Pinned independent Peer Protocol examples; expected bytes do not use production encoders.
pub(super) const CONTROL: &[u8] = b"\x01\x00\x00\x00\x14{\"type\":\"board.get\"}";
pub(super) const BODY: &[u8] =
    b"\x02\x00\x00\x00\x0f\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00abc";
// Literal control/binary lengths and exact quote-source hash remain independent of the implementation.
#[test]
fn pinned_lengths_and_types() {
    assert_eq!(CONTROL.len(), 25);
    assert_eq!(BODY.len(), 20);
    assert_eq!(&CONTROL[5..], br#"{"type":"board.get"}"#);
    let values: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/values.json")).unwrap();
    assert_eq!(values["uint64"], "18446744073709551615");
    assert_eq!(values["float"]["$float"], "80000000");
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(include_bytes!("fixtures/quote.json"));
    let expected = include_str!("fixtures/quote.sha256").trim();
    assert_eq!(
        hash.iter().map(|b| format!("{b:02x}")).collect::<String>(),
        expected
    );
}
