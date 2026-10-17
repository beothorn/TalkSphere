use super::*;
use crate::modules::transport::{config::MAX_FRAME_BYTES, test_support::vectors};

// Independent literal expectations from specification sections 3–4, not encode().
const DATA_ONE: [u8; 12] = [1, 0x11, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1];
const REGISTRY: [(u8, FrameType); 10] = [
    (0x01, FrameType::Hello),
    (0x02, FrameType::Auth),
    (0x03, FrameType::Ping),
    (0x04, FrameType::Pong),
    (0x05, FrameType::GoAway),
    (0x06, FrameType::Error),
    (0x10, FrameType::Open),
    (0x11, FrameType::Data),
    (0x12, FrameType::End),
    (0x13, FrameType::Cancel),
];

#[test]
fn upstream_ping_and_pong_headers_match_in_both_directions() {
    for (occurrence, kind) in [FrameType::Ping, FrameType::Pong].into_iter().enumerate() {
        let expected = vectors::bytes("header", occurrence);
        let header = FrameHeader::new(kind, 0, 8).unwrap();
        assert_eq!(header.encode().as_slice(), expected);
        assert_eq!(FrameHeader::decode(&expected), Ok(header));
        assert_eq!(
            header.frame_len(FrameProtection::Tagged).unwrap(),
            vectors::bytes("complete bytes written to TCP", occurrence).len()
        );
    }
}

#[test]
fn tr03_data_boundary_sizes_have_exact_big_endian_bytes() {
    for (length, expected) in [
        (1, DATA_ONE),
        (65_536, [1, 0x11, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0]),
    ] {
        let header = FrameHeader::new(FrameType::Data, 1, length).unwrap();
        assert_eq!(header.encode(), expected);
        let decoded = FrameHeader::decode(&expected).unwrap();
        assert_eq!(decoded.frame_type(), FrameType::Data);
        assert_eq!(decoded.stream_id(), 1);
        assert_eq!(decoded.payload_len(), length);
        assert_eq!(
            decoded.frame_len(FrameProtection::Untagged).unwrap(),
            12 + length as usize
        );
        assert_eq!(
            decoded.frame_len(FrameProtection::Tagged).unwrap(),
            28 + length as usize
        );
    }
    assert_eq!(
        FrameHeader::new(FrameType::Data, 1, 65_536)
            .unwrap()
            .frame_len(FrameProtection::Tagged)
            .unwrap(),
        MAX_FRAME_BYTES
    );
}

#[test]
fn multibyte_stream_ids_and_lengths_preserve_every_byte() {
    let bytes = [1, 0x11, 0, 0, 0x12, 0x34, 0x56, 0x78, 0, 0, 0x12, 0x34];
    let header = FrameHeader::decode(&bytes).unwrap();
    assert_eq!(header.stream_id(), 0x1234_5678);
    assert_eq!(header.payload_len(), 0x1234);
    assert_eq!(header.encode(), bytes);
    let end = FrameHeader::new(FrameType::End, u32::MAX, 0).unwrap();
    assert_eq!(
        end.encode(),
        [1, 0x12, 0, 0, 255, 255, 255, 255, 0, 0, 0, 0]
    );
    assert_eq!(end.frame_len(FrameProtection::Untagged), Ok(12));
    assert_eq!(end.frame_len(FrameProtection::Tagged), Ok(28));
}

#[test]
fn every_registered_type_and_every_unassigned_byte_is_checked() {
    for value in 0..=u8::MAX {
        let mut bytes = DATA_ONE;
        bytes[1] = value;
        match REGISTRY.iter().find(|(wire, _)| *wire == value) {
            Some((_, kind)) => {
                assert_eq!(FrameType::try_from(value), Ok(*kind));
                if value <= 0x06 {
                    bytes[7] = 0;
                }
                let header = FrameHeader::decode(&bytes).unwrap();
                assert_eq!(header.frame_type(), *kind);
                assert_eq!(header.encode(), bytes);
            }
            None => {
                assert_eq!(
                    FrameType::try_from(value),
                    Err(HeaderError::UnknownType(value))
                );
                assert_eq!(
                    FrameHeader::decode(&bytes),
                    Err(HeaderError::UnknownType(value))
                );
            }
        }
    }
}

#[test]
fn all_unsupported_versions_and_nonzero_flags_are_rejected_exactly() {
    for version in 0..=u8::MAX {
        if version == 1 {
            continue;
        }
        let mut bytes = DATA_ONE;
        bytes[0] = version;
        assert_eq!(
            FrameHeader::decode(&bytes),
            Err(HeaderError::UnsupportedVersion(version))
        );
    }
    for flags in 1..=u16::MAX {
        let mut bytes = DATA_ONE;
        bytes[2..4].copy_from_slice(&flags.to_be_bytes());
        assert_eq!(
            FrameHeader::decode(&bytes),
            Err(HeaderError::NonzeroFlags(flags))
        );
    }
}

#[test]
fn oversized_payloads_fail_from_the_header_alone_and_cannot_be_encoded() {
    for length in [65_537, 0x0100_0000, u32::MAX] {
        let mut bytes = DATA_ONE;
        bytes[8..12].copy_from_slice(&length.to_be_bytes());
        assert_eq!(
            FrameHeader::decode(&bytes),
            Err(HeaderError::PayloadTooLarge(length))
        );
        assert_eq!(
            FrameHeader::new(FrameType::Data, 1, length),
            Err(HeaderError::PayloadTooLarge(length))
        );
    }
}

#[test]
fn static_type_stream_id_rules_apply_to_decoding_and_construction() {
    for (value, kind) in REGISTRY {
        for stream_id in [0u32, 1, 2, u32::MAX] {
            let valid = match value {
                0x01..=0x05 => stream_id == 0,
                0x06 => true,
                _ => stream_id != 0,
            };
            let mut bytes = DATA_ONE;
            bytes[1] = value;
            bytes[4..8].copy_from_slice(&stream_id.to_be_bytes());
            let decoded = FrameHeader::decode(&bytes);
            let constructed = FrameHeader::new(kind, stream_id, 1);
            if valid {
                assert!(decoded.is_ok());
                assert_eq!(decoded, constructed);
            } else {
                let error = Err(HeaderError::InvalidStreamId {
                    frame_type: kind,
                    stream_id,
                });
                assert_eq!(decoded, error);
                assert_eq!(constructed, error);
            }
        }
    }
}

#[test]
fn incomplete_or_trailing_bytes_are_not_treated_as_a_header() {
    for length in 0..12 {
        assert_eq!(
            FrameHeader::decode(&DATA_ONE[..length]),
            Err(HeaderError::InvalidLength { actual: length })
        );
    }
    for length in [13, 24, 36] {
        let mut bytes = vec![0; length];
        bytes[..12].copy_from_slice(&DATA_ONE);
        assert_eq!(
            FrameHeader::decode(&bytes),
            Err(HeaderError::InvalidLength { actual: length })
        );
    }
    let mut invalid = DATA_ONE;
    invalid[0] = 2;
    let combined = [invalid, DATA_ONE].concat();
    assert_eq!(
        FrameHeader::decode(&combined),
        Err(HeaderError::InvalidLength { actual: 24 })
    );
    assert_eq!(
        FrameHeader::decode(&combined[..12]),
        Err(HeaderError::UnsupportedVersion(2))
    );
}

#[test]
fn unsupported_version_remains_distinct_even_when_other_fields_are_invalid() {
    let mut bytes = [255; 12];
    bytes[0] = 2;
    assert_eq!(
        FrameHeader::decode(&bytes),
        Err(HeaderError::UnsupportedVersion(2))
    );
    bytes[0] = 1;
    assert_eq!(
        FrameHeader::decode(&bytes),
        Err(HeaderError::NonzeroFlags(u16::MAX))
    );
    bytes[2..4].fill(0);
    assert_eq!(
        FrameHeader::decode(&bytes),
        Err(HeaderError::PayloadTooLarge(u32::MAX))
    );
    bytes[8..12].fill(0);
    assert_eq!(
        FrameHeader::decode(&bytes),
        Err(HeaderError::UnknownType(255))
    );
}

#[test]
fn preliminary_headers_do_not_decide_payload_or_stream_state_validity() {
    // Payload shape errors on existing streams have different error scope;
    // do not turn them into header failures before integrity/state validation.
    for (kind, length) in [(FrameType::Data, 0), (FrameType::End, 1)] {
        let header = FrameHeader::new(kind, 2, length).unwrap();
        assert_eq!(FrameHeader::decode(&header.encode()), Ok(header));
    }
    // Existence and parity checks need connection state; ERROR permits either scope.
    assert!(FrameHeader::new(FrameType::Error, u32::MAX, 4).is_ok());
}
