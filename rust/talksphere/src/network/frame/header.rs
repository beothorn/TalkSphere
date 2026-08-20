use tracing::trace;

pub const FRAME_HEADER_LENGTH: usize = 12;
const FLAGS_START_INDEX: usize = 2;
const STREAM_ID_START_INDEX: usize = 4;
const PAYLOAD_LENGTH_START_INDEX: usize = 8;

/// This represents the fixed-size metadata placed before every CR2SE payload.
#[derive(Debug, PartialEq, Eq)]
pub struct FrameHeader {
    pub version: u8,
    pub frame_type: u8,
    pub flags: u16,
    pub stream_id: u32,
    pub payload_length: u32,
}

impl FrameHeader {
    pub fn encode(&self) -> [u8; FRAME_HEADER_LENGTH] {
        trace!(">FrameHeader::encode(): encoding a CR2SE frame header");

        let mut encoded_header = [0_u8; FRAME_HEADER_LENGTH];
        encoded_header[0] = self.version;
        encoded_header[1] = self.frame_type;
        encoded_header[FLAGS_START_INDEX..STREAM_ID_START_INDEX]
            .copy_from_slice(&self.flags.to_be_bytes());
        encoded_header[STREAM_ID_START_INDEX..PAYLOAD_LENGTH_START_INDEX]
            .copy_from_slice(&self.stream_id.to_be_bytes());
        encoded_header[PAYLOAD_LENGTH_START_INDEX..FRAME_HEADER_LENGTH]
            .copy_from_slice(&self.payload_length.to_be_bytes());

        trace!("<FrameHeader::encode(): returning the encoded CR2SE frame header");
        encoded_header
    }
}

#[cfg(test)]
mod tests {
    use super::{FRAME_HEADER_LENGTH, FrameHeader};

    const PROTOCOL_VERSION: u8 = 1;
    const EXAMPLE_FRAME_TYPE: u8 = 2;
    const NO_FLAGS: u16 = 0;
    const EXAMPLE_STREAM_ID: u32 = 17;
    const EXAMPLE_PAYLOAD_LENGTH: u32 = 5;

    #[test]
    fn encode_matches_the_protocol_example() {
        tracing::trace!(
            ">encode_matches_the_protocol_example(): checking the specification example"
        );

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
}
