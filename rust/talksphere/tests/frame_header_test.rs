use talksphere::network::frame::{FRAME_HEADER_LENGTH, FrameHeader};

const PROTOCOL_VERSION: u8 = 1;
const EXAMPLE_FRAME_TYPE: u8 = 2;
const NO_FLAGS: u16 = 0;
const EXAMPLE_STREAM_ID: u32 = 17;
const EXAMPLE_PAYLOAD_LENGTH: u32 = 5;

#[test]
fn encode_matches_the_protocol_example() {
    tracing::trace!(">encode_matches_the_protocol_example(): checking the specification example");

    let frame_header = FrameHeader {
        version: PROTOCOL_VERSION,
        frame_type: EXAMPLE_FRAME_TYPE,
        flags: NO_FLAGS,
        stream_id: EXAMPLE_STREAM_ID,
        payload_length: EXAMPLE_PAYLOAD_LENGTH,
    };
    let expected_header: [u8; FRAME_HEADER_LENGTH] = [
        0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x11, 0x00, 0x00, 0x00, 0x05,
    ];

    assert_eq!(frame_header.encode(), expected_header);

    tracing::trace!(
        "<encode_matches_the_protocol_example(): the encoded bytes match the specification"
    );
}

#[test]
fn decode_restores_the_protocol_example_fields() {
    tracing::trace!(
        ">decode_restores_the_protocol_example_fields(): decoding the specification example"
    );

    let encoded_header: [u8; FRAME_HEADER_LENGTH] = [
        0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x11, 0x00, 0x00, 0x00, 0x05,
    ];
    let expected_header = FrameHeader {
        version: PROTOCOL_VERSION,
        frame_type: EXAMPLE_FRAME_TYPE,
        flags: NO_FLAGS,
        stream_id: EXAMPLE_STREAM_ID,
        payload_length: EXAMPLE_PAYLOAD_LENGTH,
    };

    assert_eq!(FrameHeader::decode(encoded_header), expected_header);

    tracing::trace!(
        "<decode_restores_the_protocol_example_fields(): all decoded fields match the specification"
    );
}
