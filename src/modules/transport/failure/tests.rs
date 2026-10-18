//! Failure-scope matrix; socket closure is exercised by connection composition.
use super::*;

// Unsupported versions, truncation, failed tags and ambiguous phases never authorize a reply.
#[test]
fn silent_connection_failures() {
    for error in [
        FramingError::Header(HeaderError::UnsupportedVersion(2)),
        FramingError::Truncated,
        FramingError::Io(std::io::ErrorKind::BrokenPipe),
        FramingError::Closed,
        FramingError::PhaseChanged,
        FramingError::Shape,
        FramingError::Busy,
    ] {
        assert!(Disposition::framing(error).response.is_none());
    }
    assert_eq!(
        Disposition::framing(FramingError::Truncated).failure.code,
        FailureCode::TruncatedFrame
    );
    for error in [
        IntegrityError::Shape,
        IntegrityError::Dependency(super::super::ports::DependencyError::VerificationFailed),
        IntegrityError::Exhausted,
    ] {
        assert!(Disposition::integrity(error).response.is_none());
    }
    assert_eq!(
        Disposition::integrity(IntegrityError::Exhausted)
            .failure
            .code,
        FailureCode::SequenceExhausted
    );
}

// Safe header and handshake errors preserve wire codes; uncertainty suppresses even those responses.
#[test]
fn response_codes_and_output_gate() {
    for (error, code) in [
        (
            FramingError::Header(HeaderError::PayloadTooLarge(65537)),
            ErrorCode::FrameSizeError,
        ),
        (
            FramingError::Header(HeaderError::NonzeroFlags(1)),
            ErrorCode::ProtocolError,
        ),
    ] {
        let disposition = Disposition::framing(error);
        assert_eq!(disposition.with_safe_output(true).response, Some(code));
        assert_eq!(disposition.with_safe_output(false).response, None);
    }
    assert_eq!(
        Disposition::handshake(HandshakeError::UnexpectedIdentity).response,
        Some(ErrorCode::AuthenticationFailed)
    );
    assert_eq!(
        Disposition::handshake(HandshakeError::Order).response,
        Some(ErrorCode::ProtocolError)
    );
    assert_eq!(
        Disposition::handshake(HandshakeError::Framing(FramingError::Truncated)).response,
        None
    );
}

// A peer ERROR is terminal information, not a request for an answering ERROR.
#[test]
fn peer_handshake_error_keeps_origin() {
    let disposition = Disposition::handshake(HandshakeError::Peer(ErrorCode::Cancelled));
    assert_eq!(
        disposition.failure,
        Failure {
            origin: FailureOrigin::Peer,
            code: FailureCode::Cancelled
        }
    );
    assert_eq!(disposition.response, None);
}
