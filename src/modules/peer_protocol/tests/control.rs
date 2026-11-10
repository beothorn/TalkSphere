//! Wire control rejection and full-size/depth acceptance independent of Board's stricter document depth.
use super::{Error, control::Control, record::CONTROL_MAX};
// Duplicate escaped names, trailing documents, bad UTF-8/type and excessive containers never reach handlers.
#[test]
fn control_syntax_and_bounds() {
    for bytes in [
        br#"{"type":"board.get","type":"board.get"}"#.as_slice(),
        br#"{"type":"x","t\u0079pe":"x"}"#,
        br#"{"type":"x","nested":{"a":1,"a":2}}"#,
        br#"{"type":"x"}{}"#,
        br#"{"type":null}"#,
        br#"[]"#,
        br#"{"type":""}"#,
        &[255],
        b"\xef\xbb\xbf{}",
    ] {
        assert!(Control::parse(bytes, 65536).is_err());
    }
    let raw = br#" {"type":"quote","maximumPrice":"7","board":{"price":18446744073709551615}} "#;
    let c = Control::parse(raw, 65536).unwrap();
    assert_eq!(c.bytes(), raw);
    assert_eq!(
        c.value()["board"]["price"].to_string(),
        "18446744073709551615"
    );
    for (depth, valid) in [(32, true), (33, false)] {
        let raw = format!(
            "{{\"type\":\"x\",\"v\":{}0{}}}",
            "[".repeat(depth - 1),
            "]".repeat(depth - 1)
        );
        assert_eq!(Control::parse(raw.as_bytes(), 65536).is_ok(), valid);
    }
    let raw = format!(
        "{{\"type\":\"x\",\"v\":\"{}\"}}",
        "a".repeat(CONTROL_MAX - 19)
    );
    assert_eq!(raw.len(), CONTROL_MAX);
    assert!(Control::parse(raw.as_bytes(), 65536).is_ok());
    assert!(matches!(
        Control::parse(raw.as_bytes(), 1),
        Err(Error::Limit)
    ));
}
