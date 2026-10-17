use super::{
    FrameType, HEADER_BYTES, HeaderError, INTEGRITY_TAG_BYTES, MAX_PAYLOAD_BYTES, VERSION,
};

/// Direction-specific wire overhead, supplied by the future handshake driver.
/// Selecting Tagged does not authenticate a frame or connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameProtection {
    Untagged,
    Tagged,
}

/// Immutable, preliminarily validated version 1 header.
///
/// Payload lengths exclude both header and tag. Type-specific payload rules,
/// stream existence/parity/sequence and authentication are checked separately.
///
/// ```
/// use talksphere::modules::transport::codec::{FrameHeader, FrameProtection, FrameType};
/// let header = FrameHeader::new(FrameType::Data, 1, 65_536)?;
/// assert_eq!(header.encode(), [1, 0x11, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0]);
/// assert_eq!(header.frame_len(FrameProtection::Tagged)?, 65_564);
/// # Ok::<(), talksphere::modules::transport::codec::HeaderError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    frame_type: FrameType,
    stream_id: u32,
    payload_len: u32,
}

impl FrameHeader {
    pub fn new(
        frame_type: FrameType,
        stream_id: u32,
        payload_len: u32,
    ) -> Result<Self, HeaderError> {
        Self::validate_payload_len(payload_len)?;
        frame_type.validate_stream_id(stream_id)?;
        Ok(Self {
            frame_type,
            stream_id,
            payload_len,
        })
    }

    /// Decode exactly one header without allocating or reading a payload.
    /// Rejects short or trailing input; never scans for a replacement boundary.
    pub fn decode(bytes: &[u8]) -> Result<Self, HeaderError> {
        let bytes: &[u8; HEADER_BYTES] =
            bytes.try_into().map_err(|_| HeaderError::InvalidLength {
                actual: bytes.len(),
            })?;
        if bytes[0] != VERSION {
            return Err(HeaderError::UnsupportedVersion(bytes[0]));
        }
        let flags = u16::from_be_bytes([bytes[2], bytes[3]]);
        if flags != 0 {
            return Err(HeaderError::NonzeroFlags(flags));
        }
        let payload_len = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        // Preserve the pseudocode's version/flags/size/type validation order.
        Self::validate_payload_len(payload_len)?;
        let frame_type = FrameType::try_from(bytes[1])?;
        let stream_id = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        Self::new(frame_type, stream_id, payload_len)
    }

    pub fn encode(self) -> [u8; HEADER_BYTES] {
        let mut bytes = [0; HEADER_BYTES];
        bytes[0] = VERSION;
        bytes[1] = self.frame_type as u8;
        bytes[4..8].copy_from_slice(&self.stream_id.to_be_bytes());
        bytes[8..12].copy_from_slice(&self.payload_len.to_be_bytes());
        bytes
    }

    pub fn frame_type(self) -> FrameType {
        self.frame_type
    }
    pub fn stream_id(self) -> u32 {
        self.stream_id
    }
    pub fn payload_len(self) -> u32 {
        self.payload_len
    }

    /// Checked total size; the caller supplies the current direction's tag state.
    /// This performs no allocation and does not validate handshake sequencing.
    pub fn frame_len(self, protection: FrameProtection) -> Result<usize, HeaderError> {
        let payload = usize::try_from(self.payload_len).map_err(|_| HeaderError::SizeOverflow)?;
        let tag = match protection {
            FrameProtection::Untagged => 0,
            FrameProtection::Tagged => INTEGRITY_TAG_BYTES,
        };
        HEADER_BYTES
            .checked_add(payload)
            .and_then(|size| size.checked_add(tag))
            .ok_or(HeaderError::SizeOverflow)
    }

    fn validate_payload_len(length: u32) -> Result<(), HeaderError> {
        if u64::from(length) > MAX_PAYLOAD_BYTES as u64 {
            Err(HeaderError::PayloadTooLarge(length))
        } else {
            Ok(())
        }
    }
}
