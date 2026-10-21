//! Secret-free local Identity failures; no error embeds caller input or private storage details.

/// Observable local failures, separate from peer protocol error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    /// Malformed local input or invalid configuration.
    InvalidInput,
    /// Unsupported local contract, format, algorithm or platform.
    Unsupported,
    /// The claimed identity does not match its key.
    IdentityMismatch,
    /// A signature or credential binding was rejected.
    InvalidProof,
    /// Local authority does not permit this operation.
    AccessDenied,
    /// The protected dependency cannot currently serve the request.
    Unavailable,
    /// Input exceeds immutable local policy.
    ResourceLimit,
    /// Installing or replacing state would conflict with existing authority.
    Conflict,
    /// Installation may have committed; reconcile before retrying.
    OutcomeUnknown,
}
impl std::fmt::Display for IdentityError {
    // Render a fixed explanation without exposing existence or protected path details.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "invalid identity input",
            Self::Unsupported => "unsupported identity operation",
            Self::IdentityMismatch => "identity does not match the supplied key",
            Self::InvalidProof => "identity proof or binding rejected",
            Self::AccessDenied => "identity authority denied",
            Self::Unavailable => "identity dependency unavailable",
            Self::ResourceLimit => "identity resource limit exceeded",
            Self::Conflict => "identity state conflict",
            Self::OutcomeUnknown => {
                "identity installation outcome unknown; reconcile before retrying"
            }
        })
    }
}
impl std::error::Error for IdentityError {}
