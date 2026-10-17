//! Wire-format tests from TR-11/12/13 and independent section 10/11 examples.

use super::*;
use crate::modules::transport::test_support::vectors;

// A caller can echo the upstream token unchanged, including arbitrary binary data.
#[test]
fn ping_and_pong_match_the_upstream_tokens() {
    for occurrence in 0..2 {
        let bytes = vectors::bytes("payload", occurrence);
        let token = PingToken::decode(&bytes).unwrap();
        assert_eq!(token.encode(), [0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(PingToken::new([255; 8]).encode(), [255; 8]);
    }
    for length in [0, 1, 7, 9, 16, 65_536] {
        assert_eq!(
            PingToken::decode(&vec![0; length]).err(),
            Some(PayloadError::LengthMismatch {
                expected: 8,
                actual: length
            })
        );
    }
}

// Explicit registry expectations prevent a local encoder/decoder agreement hiding wrong codes.
#[test]
fn every_registered_and_unregistered_code_is_checked() {
    let codes = [
        ErrorCode::NoError,
        ErrorCode::ProtocolError,
        ErrorCode::AuthenticationFailed,
        ErrorCode::FrameSizeError,
        ErrorCode::UnsupportedProtocol,
        ErrorCode::StreamStateError,
        ErrorCode::RefusedStream,
        ErrorCode::InternalError,
        ErrorCode::IdExhausted,
        ErrorCode::Cancelled,
        ErrorCode::ResourceLimit,
    ];
    for raw in 0..=u16::MAX {
        let code_bytes = raw.to_be_bytes();
        let error_bytes = [code_bytes[0], code_bytes[1], 0, 0];
        let goaway_bytes = [0, 0, 0, 0, code_bytes[0], code_bytes[1], 0, 0];
        if let Some(&code) = codes.get(usize::from(raw)) {
            assert_eq!(ErrorCode::try_from(raw), Ok(code));
            assert_eq!(code as u16, raw);
            assert_eq!(GoAway::new(0, code, "").unwrap().encode(), goaway_bytes);
            assert_eq!(GoAway::decode(&goaway_bytes).unwrap().code(), code);
            if raw == 0 {
                assert_eq!(
                    ErrorPayload::new(code, "").err(),
                    Some(PayloadError::NoErrorForbidden)
                );
                assert_eq!(
                    ErrorPayload::decode(&error_bytes).err(),
                    Some(PayloadError::NoErrorForbidden)
                );
            } else {
                assert_eq!(ErrorPayload::new(code, "").unwrap().encode(), error_bytes);
                assert_eq!(ErrorPayload::decode(&error_bytes).unwrap().code(), code);
            }
        } else {
            assert_eq!(
                ErrorCode::try_from(raw),
                Err(PayloadError::UnknownCode(raw))
            );
            assert_eq!(
                ErrorPayload::decode(&error_bytes).err(),
                Some(PayloadError::UnknownCode(raw))
            );
            assert_eq!(
                GoAway::decode(&goaway_bytes).err(),
                Some(PayloadError::UnknownCode(raw))
            );
        }
    }
}

// An independently written UTF-8 example checks byte lengths and preservation, not just round trips.
#[test]
fn error_and_cancel_preserve_exact_utf8_diagnostics() {
    let expected = [0, 9, 0, 3, b'o', 0xc3, 0xb9];
    let payload = ErrorPayload::new(ErrorCode::Cancelled, "où").unwrap();
    assert_eq!(payload.encode(), expected);
    let decoded = ErrorPayload::decode(&expected).unwrap();
    assert_eq!(decoded.code(), ErrorCode::Cancelled);
    assert_eq!(decoded.reason(), "où");
    assert_eq!(decoded.reason().as_ptr(), expected[4..].as_ptr());
    // Diagnostic text that resembles a code never changes the actual code.
    assert_eq!(
        ErrorPayload::new(ErrorCode::ProtocolError, "NO_ERROR")
            .unwrap()
            .code(),
        ErrorCode::ProtocolError
    );
}

// GOAWAY's full-width ID and code are retained; parity/opened-ID checks need future connection state.
#[test]
fn goaway_encodes_exact_fields_without_inventing_state_rules() {
    let expected = [0x12, 0x34, 0x56, 0x78, 0, 0, 0, 3, b'b', b'y', b'e'];
    assert_eq!(
        GoAway::new(0x1234_5678, ErrorCode::NoError, "bye")
            .unwrap()
            .encode(),
        expected
    );
    let decoded = GoAway::decode(&expected).unwrap();
    assert_eq!(decoded.last_accepted_remote_stream_id(), 0x1234_5678);
    assert_eq!(decoded.code(), ErrorCode::NoError);
    assert_eq!(decoded.reason(), "bye");
    assert_eq!(decoded.reason().as_ptr(), expected[8..].as_ptr());
    assert_eq!(
        GoAway::new(u32::MAX, ErrorCode::IdExhausted, "")
            .unwrap()
            .encode(),
        [255, 255, 255, 255, 0, 8, 0, 0]
    );
    assert_eq!(
        GoAway::decode(&[0, 0, 0, 3, 0, 0, 0, 0])
            .unwrap()
            .last_accepted_remote_stream_id(),
        3
    );
}

// Both payload families accept exactly 1,024 bytes, even when text has fewer characters.
#[test]
fn reason_limits_count_utf8_bytes_and_validate_before_copying() {
    let reason = "é".repeat(512);
    let error = ErrorPayload::new(ErrorCode::ResourceLimit, &reason)
        .unwrap()
        .encode();
    assert_eq!(&error[..4], [0, 10, 4, 0]);
    assert_eq!(error.len(), 1_028);
    assert_eq!(ErrorPayload::decode(&error).unwrap().reason(), reason);
    let goaway = GoAway::new(3, ErrorCode::ResourceLimit, &reason)
        .unwrap()
        .encode();
    assert_eq!(&goaway[..8], [0, 0, 0, 3, 0, 10, 4, 0]);
    assert_eq!(goaway.len(), 1_032);
    assert_eq!(GoAway::decode(&goaway).unwrap().reason(), reason);
    for reason in [format!("{reason}x"), "é".repeat(513)] {
        let expected = Some(PayloadError::ReasonTooLong {
            actual: reason.len(),
        });
        assert_eq!(
            ErrorPayload::new(ErrorCode::Cancelled, &reason).err(),
            expected
        );
        assert_eq!(GoAway::new(0, ErrorCode::NoError, &reason).err(), expected);
    }
    // Only the prefix is needed to reject an oversized claim, without a text buffer.
    for declared in [1_025u16, u16::MAX] {
        let length = declared.to_be_bytes();
        let expected = Some(PayloadError::ReasonTooLong {
            actual: usize::from(declared),
        });
        assert_eq!(
            ErrorPayload::decode(&[0, 1, length[0], length[1]]).err(),
            expected
        );
        assert_eq!(
            GoAway::decode(&[0, 0, 0, 0, 0, 0, length[0], length[1]]).err(),
            expected
        );
    }
}

// Every incomplete prefix and any extra byte is malformed, rather than silently discarded.
#[test]
fn truncation_and_trailing_bytes_are_rejected() {
    let error = [0, 1, 0, 3, b'b', b'a', b'd'];
    let goaway = [0, 0, 0, 3, 0, 0, 0, 3, b'b', b'y', b'e'];
    for length in 0..error.len() {
        let expected = if length < 4 {
            PayloadError::TooShort {
                minimum: 4,
                actual: length,
            }
        } else {
            PayloadError::LengthMismatch {
                expected: 7,
                actual: length,
            }
        };
        assert_eq!(ErrorPayload::decode(&error[..length]).err(), Some(expected));
    }
    for length in 0..goaway.len() {
        let expected = if length < 8 {
            PayloadError::TooShort {
                minimum: 8,
                actual: length,
            }
        } else {
            PayloadError::LengthMismatch {
                expected: 11,
                actual: length,
            }
        };
        assert_eq!(GoAway::decode(&goaway[..length]).err(), Some(expected));
    }
    assert_eq!(
        ErrorPayload::decode(&[error.as_slice(), &[0]].concat()).err(),
        Some(PayloadError::LengthMismatch {
            expected: 7,
            actual: 8
        })
    );
    assert_eq!(
        GoAway::decode(&[goaway.as_slice(), &[0]].concat()).err(),
        Some(PayloadError::LengthMismatch {
            expected: 11,
            actual: 12
        })
    );
}

// Invalid UTF-8 must fail without replacement characters, including overlong and incomplete sequences.
#[test]
fn malformed_utf8_is_not_repaired() {
    for invalid in [
        vec![0xff],
        vec![0xc0, 0x80],
        vec![0xc3],
        vec![0xed, 0xa0, 0x80],
    ] {
        let mut bytes = vec![0, 1, 0, invalid.len() as u8];
        bytes.extend(invalid);
        assert_eq!(
            ErrorPayload::decode(&bytes).err(),
            Some(PayloadError::InvalidUtf8)
        );
        let goaway = [&[0, 0, 0, 0], bytes.as_slice()].concat();
        assert_eq!(
            GoAway::decode(&goaway).err(),
            Some(PayloadError::InvalidUtf8)
        );
    }
}

// Regression for CR2SE bug 0001: valid UTF-8 is insufficient when the field starts with a BOM.
#[test]
fn leading_byte_order_marks_are_rejected_but_interior_characters_are_preserved() {
    let bytes = [0, 1, 0, 3, 0xef, 0xbb, 0xbf];
    assert!(std::str::from_utf8(&bytes[4..]).is_ok());
    assert_eq!(
        ErrorPayload::decode(&bytes).err(),
        Some(PayloadError::ByteOrderMark)
    );
    assert_eq!(
        GoAway::decode(&[&[0, 0, 0, 0], bytes.as_slice()].concat()).err(),
        Some(PayloadError::ByteOrderMark)
    );
    assert_eq!(
        ErrorPayload::new(ErrorCode::ProtocolError, "\u{feff}").err(),
        Some(PayloadError::ByteOrderMark)
    );
    assert_eq!(
        GoAway::new(0, ErrorCode::NoError, "\u{feff}").err(),
        Some(PayloadError::ByteOrderMark)
    );
    let reason = "a\u{feff}\0\n";
    let encoded = ErrorPayload::new(ErrorCode::ProtocolError, reason)
        .unwrap()
        .encode();
    assert_eq!(ErrorPayload::decode(&encoded).unwrap().reason(), reason);
}

// Human-readable error messages retain safe numeric context and never echo input text.
#[test]
fn payload_errors_explain_rejections_without_diagnostic_text() {
    let cases = [
        (
            PayloadError::TooShort {
                minimum: 8,
                actual: 2,
            },
            "payload needs at least 8 bytes, got 2",
        ),
        (
            PayloadError::LengthMismatch {
                expected: 8,
                actual: 9,
            },
            "payload needs exactly 8 bytes, got 9",
        ),
        (
            PayloadError::ReasonTooLong { actual: 1025 },
            "diagnostic has 1025 bytes; maximum is 1024",
        ),
        (PayloadError::InvalidUtf8, "diagnostic is not valid UTF-8"),
        (
            PayloadError::ByteOrderMark,
            "diagnostic begins with a byte-order mark",
        ),
        (
            PayloadError::UnknownCode(11),
            "unknown control error code: 0x000b",
        ),
        (
            PayloadError::NoErrorForbidden,
            "NO_ERROR is only permitted in GOAWAY",
        ),
    ];
    for (error, message) in cases {
        let error: &dyn std::error::Error = &error;
        assert_eq!(error.to_string(), message);
        assert!(error.source().is_none());
    }
}
