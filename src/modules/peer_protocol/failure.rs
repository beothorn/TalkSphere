//! Specified failure codes preserve diagnostics without treating interrupted or committed work as uncharged rollback.
use super::{
    Error,
    authority::{Action, Authority},
    contract::{Association, Context},
    message::{Kind, Message},
    record::Record,
};
use crate::modules::services::{
    self,
    persistence::Persistence,
    runtime::{Coordinator, Decision, Phase},
};
use serde_json::json;
/// Known machine-readable failure meaning; unfamiliar remote codes remain generic and never authorize automatic retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    /// Malformed fields, invalid value or forbidden operation.
    InvalidRequest,
    /// Unsupported service, version, mode or settlement.
    Unsupported,
    /// Missing/inactive resource or concealed access failure.
    NotAvailable,
    /// Explicit access refusal without existence disclosure.
    NotAuthorized,
    /// Current contract or quote no longer matches.
    TermsChanged,
    /// Price bound or exact fee agreement rejected.
    PriceRejected,
    /// Insufficient finite credit/issuance capacity.
    InsufficientCredits,
    /// Size, time, concurrency or other finite capacity exhausted.
    ResourceLimit,
    /// Stale expiration or conflicting replay identity.
    Conflict,
    /// Provider failed without successful output.
    InternalError,
    /// Unknown remote code, treated only as a generic failure.
    Unknown,
}
impl Code {
    /// Decode known code spellings without interpreting diagnostic text or unfamiliar codes as retry instructions.
    pub fn parse(value: &str) -> Self {
        tracing::trace!("Decoding a peer failure machine code");
        match value {
            "invalid_request" => Self::InvalidRequest,
            "unsupported" => Self::Unsupported,
            "not_available" => Self::NotAvailable,
            "not_authorized" => Self::NotAuthorized,
            "terms_changed" => Self::TermsChanged,
            "price_rejected" => Self::PriceRejected,
            "insufficient_credits" => Self::InsufficientCredits,
            "resource_limit" => Self::ResourceLimit,
            "conflict" => Self::Conflict,
            "internal_error" => Self::InternalError,
            _ => Self::Unknown,
        }
    }
    /// Obtain only a defined version-one spelling; unknown received values are not reflected onto another peer.
    pub fn wire(self) -> Option<&'static str> {
        tracing::trace!("Reading a defined peer failure spelling");
        Some(match self {
            Self::InvalidRequest => "invalid_request",
            Self::Unsupported => "unsupported",
            Self::NotAvailable => "not_available",
            Self::NotAuthorized => "not_authorized",
            Self::TermsChanged => "terms_changed",
            Self::PriceRejected => "price_rejected",
            Self::InsufficientCredits => "insufficient_credits",
            Self::ResourceLimit => "resource_limit",
            Self::Conflict => "conflict",
            Self::InternalError => "internal_error",
            Self::Unknown => return None,
        })
    }
    /// Classify local typed refusal; use concealed=true when access denial must not expose resource existence.
    /// # Errors
    /// Unknown effects cannot become a failure claiming uncharged rollback; owner-confirmed abort is independently required.
    pub fn classify(error: Error, concealed: bool) -> Result<Self, Error> {
        tracing::trace!("Classifying a local refusal without erasing outcome uncertainty");
        Ok(match error {
            Error::Invalid => Self::InvalidRequest,
            Error::Unsupported => Self::Unsupported,
            Error::Denied if concealed => Self::NotAvailable,
            Error::Denied => Self::NotAuthorized,
            Error::Limit | Error::Cancelled => Self::ResourceLimit,
            Error::Conflict => Self::Conflict,
            Error::Insufficient => Self::InsufficientCredits,
            Error::Storage | Error::Unavailable => Self::InternalError,
            Error::Unknown => return Err(Error::Unknown),
        })
    }
    /// Map a completed failure to a local category while the separately retained Code preserves its exact known meaning.
    pub fn error(self) -> Error {
        tracing::trace!("Mapping a complete peer failure to a local category");
        match self {
            Self::InvalidRequest => Error::Invalid,
            Self::Unsupported => Error::Unsupported,
            Self::NotAvailable | Self::InternalError | Self::Unknown => Error::Unavailable,
            Self::NotAuthorized => Error::Denied,
            Self::TermsChanged | Self::Conflict => Error::Conflict,
            Self::PriceRejected | Self::InsufficientCredits => Error::Insufficient,
            Self::ResourceLimit => Error::Limit,
        }
    }
}
/// Bounded diagnostic message; neither code nor text alone proves remote rollback or normal stream completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// Exact known meaning, or Unknown for an unfamiliar remote code.
    pub code: Code,
    /// Optional untrusted UTF-8 diagnostic of at most1024 bytes; do not log it as trusted facts.
    pub message: Option<String>,
}
impl Failure {
    /// Decode one structurally validated failure control without acting on accounting or retry policy.
    /// # Errors
    /// Other control kinds or malformed/null fields cannot become failures.
    pub fn parse(message: &Message) -> Result<Self, Error> {
        tracing::trace!("Decoding bounded peer failure without applying financial effects");
        if message.kind() != Kind::Failure {
            return Err(Error::Invalid);
        }
        let v = message.value();
        Ok(Self {
            code: Code::parse(v["code"].as_str().ok_or(Error::Invalid)?),
            message: v
                .get("message")
                .map(|v| v.as_str().map(String::from).ok_or(Error::Invalid))
                .transpose()?,
        })
    }
    /// Encode a defined code and bounded optional diagnostic; emission after acceptance requires authoritative uncommitted abort.
    /// # Errors
    /// Unknown codes cannot be emitted and oversized diagnostics fail before control allocation.
    pub fn record(&self) -> Result<Record, Error> {
        tracing::trace!("Encoding a bounded specified peer failure");
        let code = self.code.wire().ok_or(Error::Unsupported)?;
        if self.message.as_ref().is_some_and(|m| m.len() > 1024) {
            return Err(Error::Limit);
        }
        let mut value = json!({"type":"failure","code":code});
        if let Some(message) = &self.message {
            value["message"] = json!(message);
        }
        Ok(Record::Control(
            serde_json::to_vec(&value).map_err(|_| Error::Invalid)?,
        ))
    }
}
/// Explicit local grants and durable owner used to prove an accepted provider attempt was aborted before claiming failure.
pub struct AbortPorts<'a, P: Persistence> {
    /// Local decision/delivery authority, independent of peer invocation grants.
    pub coordinate: &'a services::contract::Context<'a>,
    /// Read-only final phase observation authority.
    pub observe: &'a services::contract::Context<'a>,
    /// Original local coordinator, never a peer-selected owner.
    pub runtime: &'a Coordinator<P>,
}
/// Release only owner-confirmed uncommitted provider obligations before producing a wire failure.
/// # Errors
/// Foreign roles, committed/unknown effects, failed abort or uncertainty cannot produce an uncharged failure.
pub fn abort_provider<P: Persistence>(
    context: &Context<'_>,
    authority: &Authority,
    association: Association,
    ports: AbortPorts<'_, P>,
    failure: Failure,
) -> Result<Failure, Error> {
    tracing::trace!("Confirming provider abort before reporting an uncharged failure");
    context.check(authority, association.local, Action::Invoke, 0)?;
    if ports.runtime.local_identity() != association.local
        || association.operation.requester != association.peer
        || association.local == association.peer
    {
        return Err(Error::Denied);
    }
    failure.record()?;
    let key = super::acceptance::peer(association).key;
    ports
        .runtime
        .decide(ports.coordinate, key, Decision::Abort)?;
    ports.runtime.abort(ports.coordinate, key)?;
    if ports.runtime.status(ports.observe, key)? != Some(Phase::Aborted) {
        return Err(Error::Unknown);
    }
    Ok(failure)
}
