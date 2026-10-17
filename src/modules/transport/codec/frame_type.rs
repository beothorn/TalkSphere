use super::HeaderError;

/// The complete version 1 registry; application extensions cannot add types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    Hello = 0x01,
    Auth = 0x02,
    Ping = 0x03,
    Pong = 0x04,
    GoAway = 0x05,
    Error = 0x06,
    Open = 0x10,
    Data = 0x11,
    End = 0x12,
    Cancel = 0x13,
}

impl TryFrom<u8> for FrameType {
    type Error = HeaderError;

    fn try_from(value: u8) -> Result<Self, HeaderError> {
        match value {
            0x01 => Ok(Self::Hello),
            0x02 => Ok(Self::Auth),
            0x03 => Ok(Self::Ping),
            0x04 => Ok(Self::Pong),
            0x05 => Ok(Self::GoAway),
            0x06 => Ok(Self::Error),
            0x10 => Ok(Self::Open),
            0x11 => Ok(Self::Data),
            0x12 => Ok(Self::End),
            0x13 => Ok(Self::Cancel),
            _ => Err(HeaderError::UnknownType(value)),
        }
    }
}

impl FrameType {
    pub(super) fn validate_stream_id(self, stream_id: u32) -> Result<(), HeaderError> {
        let valid = match self {
            Self::Hello | Self::Auth | Self::Ping | Self::Pong | Self::GoAway => stream_id == 0,
            Self::Open | Self::Data | Self::End | Self::Cancel => stream_id != 0,
            // Whether a nonzero ID exists requires connection state.
            Self::Error => true,
        };
        if valid {
            Ok(())
        } else {
            Err(HeaderError::InvalidStreamId {
                frame_type: self,
                stream_id,
            })
        }
    }
}
