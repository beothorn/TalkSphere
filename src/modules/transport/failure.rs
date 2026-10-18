//! Connection failure classification for the driver; wire replies require safe output state.
use super::{
    api::{Failure, FailureCode, FailureOrigin},
    codec::{HeaderError, control::ErrorCode},
    framing::FramingError,
    handshake::HandshakeError,
    integrity::IntegrityError,
};

/// Terminal connection cause and optional best-effort wire response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disposition {
    /// Outcome delivered to every unfinished consumer.
    pub failure: Failure,
    /// Response permitted only if the driver can encode it without phase ambiguity.
    pub response: Option<ErrorCode>,
}

impl Disposition {
    /// Classify a frame reader/writer failure without attempting resynchronization.
    pub fn framing(error: FramingError) -> Self {
        tracing::trace!(
            function = "Disposition::framing",
            "Classifying frame failure"
        );
        let (code, response) = match error {
            FramingError::Header(HeaderError::UnsupportedVersion(_)) => {
                (FailureCode::ProtocolViolation, None)
            }
            FramingError::Header(HeaderError::PayloadTooLarge(_)) => {
                (FailureCode::FrameTooLarge, Some(ErrorCode::FrameSizeError))
            }
            FramingError::Header(_) => (
                FailureCode::ProtocolViolation,
                Some(ErrorCode::ProtocolError),
            ),
            FramingError::Truncated => (FailureCode::TruncatedFrame, None),
            FramingError::Io(_) | FramingError::Closed => (FailureCode::ConnectionLost, None),
            FramingError::PhaseChanged | FramingError::Shape | FramingError::Busy => {
                (FailureCode::Internal, None)
            }
        };
        Self {
            failure: Failure {
                origin: FailureOrigin::Local,
                code,
            },
            response,
        }
    }

    /// Classify handshake rejection; the driver checks direction phase before replying.
    pub fn handshake(error: HandshakeError) -> Self {
        tracing::trace!(
            function = "Disposition::handshake",
            "Classifying handshake failure"
        );
        if let HandshakeError::Peer(code) = error {
            return Self {
                failure: Failure {
                    origin: FailureOrigin::Peer,
                    code: super::stream::session::failure_code(code),
                },
                response: None,
            };
        }
        if let HandshakeError::Framing(error) = error {
            return Self::framing(error);
        }
        let code = match error {
            HandshakeError::Order | HandshakeError::Payload(_) => FailureCode::ProtocolViolation,
            _ => FailureCode::AuthenticationFailed,
        };
        Self {
            failure: Failure {
                origin: FailureOrigin::Local,
                code,
            },
            response: Some(if code == FailureCode::AuthenticationFailed {
                ErrorCode::AuthenticationFailed
            } else {
                ErrorCode::ProtocolError
            }),
        }
    }

    /// Integrity failures always close silently; never send output under suspect keys.
    pub fn integrity(error: IntegrityError) -> Self {
        tracing::trace!(
            function = "Disposition::integrity",
            "Classifying silent integrity closure"
        );
        Self {
            failure: Failure {
                origin: FailureOrigin::Local,
                code: if error == IntegrityError::Exhausted {
                    FailureCode::SequenceExhausted
                } else {
                    FailureCode::IntegrityFailed
                },
            },
            response: None,
        }
    }

    /// Suppress an otherwise permitted reply if output framing or integrity is uncertain.
    pub fn with_safe_output(mut self, safe: bool) -> Self {
        tracing::trace!(
            function = "Disposition::with_safe_output",
            "Checking safe error-response eligibility"
        );
        if !safe {
            self.response = None;
        }
        self
    }
}

#[cfg(test)]
#[path = "failure/tests.rs"]
mod tests;
