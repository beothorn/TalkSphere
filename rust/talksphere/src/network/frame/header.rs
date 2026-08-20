use tracing::trace;

/// The number of bytes occupied by every CR2SE version 1 frame header.
pub const FRAME_HEADER_LENGTH: usize = 12;
const FLAGS_START_INDEX: usize = 2;
const STREAM_ID_START_INDEX: usize = 4;
const PAYLOAD_LENGTH_START_INDEX: usize = 8;

/// The fixed-size metadata placed before every CR2SE payload.
///
/// Integer fields containing more than one byte are encoded in network byte
/// order. The header deliberately contains no payload data, keeping framing
/// independent from the application protocol carried by a stream.
#[derive(Debug, PartialEq, Eq)]
pub struct FrameHeader {
    /// Identifies which CR2SE protocol version encoded this frame.
    pub version: u8,
    /// Identifies how the receiving peer should interpret this frame.
    pub frame_type: u8,
    /// Contains optional boolean frame properties as a bit field.
    pub flags: u16,
    /// Identifies the logical conversation carried over the TCP connection.
    pub stream_id: u32,
    /// Declares exactly how many payload bytes follow this header.
    pub payload_length: u32,
}

impl FrameHeader {
    /// Encodes the header into the 12-byte representation used on the wire.
    ///
    /// Multi-byte fields are converted to big-endian byte order as required by
    /// CR2SE. The returned fixed-size array makes it impossible for this method
    /// to return an incomplete header.
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

    /// Decodes one complete 12-byte CR2SE frame header.
    ///
    /// The fixed-size input ensures callers cannot accidentally decode a
    /// partial TCP read. Multi-byte fields are converted from network byte
    /// order into Rust integers.
    pub fn decode(encoded_header: [u8; FRAME_HEADER_LENGTH]) -> Self {
        trace!(">FrameHeader::decode(): decoding a complete CR2SE frame header");

        let frame_header = Self {
            version: encoded_header[0],
            frame_type: encoded_header[1],
            flags: u16::from_be_bytes([encoded_header[2], encoded_header[3]]),
            stream_id: u32::from_be_bytes([
                encoded_header[4],
                encoded_header[5],
                encoded_header[6],
                encoded_header[7],
            ]),
            payload_length: u32::from_be_bytes([
                encoded_header[8],
                encoded_header[9],
                encoded_header[10],
                encoded_header[11],
            ]),
        };

        trace!("<FrameHeader::decode(): returning the decoded CR2SE frame header");
        frame_header
    }
}
