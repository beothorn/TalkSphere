//! Remote evidence-check contracts; expected values remain private to the requester.
use super::{
    Error, authorization::Authorized, contract::OperationKey, handler::Blob, value::Value,
};
use crate::modules::{
    board::{definition::Definition, offering::Offering},
    identity::api::IdentityId,
};

/// Installed family policy; included scopes are stable owner-issued references to existing paid entitlements.
#[derive(Clone, PartialEq, Eq)]
pub enum Mode {
    /// Only requester-local verification exists; no remote operation may be invented.
    Local,
    /// Each complete valid evidence response costs the offering's positive checkPrice.
    Paid,
    /// Existing paid work includes this finite number of accepted check attempts, with no new Ledger reservation.
    Included {
        /// Stable owner-scoped entitlement identity, independent of mutable offering/issuer/version.
        scope: Blob,
        /// Maximum accepted attempts allowed by the original family contract.
        maximum: u32,
    },
}
/// Trusted local monotonic observation; epochs distinguish clocks that cannot be compared across restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    /// Stable clock-instance identifier supplied by the platform, never a peer payload.
    pub epoch: [u8; 32],
    /// Milliseconds in that monotonic instance.
    pub milliseconds: u64,
}
/// Requester-side timing knowledge, never a provider assertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timing {
    /// Complete evidence arrived within the accepted response interval.
    Within(u64),
    /// Original timing cannot be compared reliably.
    Ambiguous,
    /// No complete valid evidence was received.
    Missing,
}
/// Public check-specific context; zero price is permitted only for included existing entitlements.
pub struct Invocation {
    /// Original authenticated requester/connection/stream key.
    pub key: OperationKey,
    /// Identity asking for evidence.
    pub requester: IdentityId,
    /// Identity owning the evidence resource.
    pub provider: IdentityId,
    /// Exact issuer selected for a separately paid check.
    pub issuer: IdentityId,
    /// Immutable selected current offering.
    pub offering: Offering,
    /// Exact known definition and check description.
    pub definition: Definition,
    /// Exact explicitly accepted quote bytes.
    pub quote: Blob,
    /// Positive check charge or zero for an included entitlement.
    pub price: u64,
    /// Accepted response interval in milliseconds.
    pub response_milliseconds: u64,
    /// Local execution deadline supplied by composition, not persistent wall-clock evidence.
    pub deadline: std::time::Instant,
}
/// Installed family challenge/evidence rules, independently of requester-held expected values.
pub trait Rules: Send + Sync {
    /// Validate the check mode and any existing paid entitlement through public family resource contracts.
    /// # Errors
    /// Absent entitlement, unsupported remote checks and inactive resources fail before admission.
    fn mode(&self, offering: &Offering, challenge: &Value) -> Result<Mode, Error>;
    /// Read the exact positive response interval from understood family terms.
    /// # Errors
    /// Invalid/overflowing or unsupported timing terms fail before consent.
    fn response_milliseconds(&self, offering: &Offering) -> Result<u64, Error>;
    /// Validate immutable challenge shape without rechecking current lease/capacity state during recovery.
    /// # Errors
    /// Malformed fields and unsupported operation semantics fail before resource lookup.
    fn shape(&self, challenge: &Value) -> Result<(), Error>;
    /// Validate authorized challenge shape, identity, offsets and resource eligibility without reading expectations.
    /// # Errors
    /// Invalid or unauthorized challenges fail without charge.
    fn challenge(&self, invocation: &Invocation, challenge: &Value) -> Result<(), Error>;
    /// Validate complete well-formed provider evidence and exact challenge association, never its correctness claim.
    /// # Errors
    /// Malformed/incomplete evidence cannot authorize settlement.
    fn evidence(&self, challenge: &Value, evidence: &Value) -> Result<(), Error>;
    /// Calculate requester-local pass/fail/inconclusive from evidence and private expectations.
    fn outcome(
        &self,
        challenge: &Value,
        evidence: Option<&Value>,
        expectations: Option<&Value>,
        timing: Timing,
    ) -> super::handler::Check;
}
/// Explicit check consent assembled by trusted peer/local composition after role authorization.
pub struct Agreement {
    /// Owner-authorized requester/provider roles and exact installed implementation.
    pub authorized: Authorized,
    /// Defined challenge only; expected bytes must never be included.
    pub challenge: Value,
    /// Exact complete agreed quote bytes supplied by the trusted peer adapter.
    pub bytes: Blob,
    /// SHA256 of the original complete quote bytes.
    pub hash: [u8; 32],
    /// Explicit total requester ceiling, including any agreed third-party fee.
    pub maximum: u64,
    /// Explicit third-party binding, absent for ordinary/included checks.
    pub payment: super::runtime::Payment,
}
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Info {
    pub mode: Mode,
    pub response: u64,
    pub sent: Option<Stamp>,
    pub received: Option<Stamp>,
}
impl Info {
    // Stable consent excludes later local timing observations while retaining original allowance/timing terms.
    pub(crate) fn binding(&self) -> Value {
        let mut copy = self.clone();
        copy.sent = None;
        copy.received = None;
        copy.value()
    }
    // Explicit local image representation is independent of any CR2SE peer record encoding.
    pub(crate) fn value(&self) -> Value {
        let (mode, scope, maximum) = match &self.mode {
            Mode::Paid => (0, Vec::new(), 0),
            Mode::Included { scope, maximum } => (1, scope.bytes().to_vec(), *maximum),
            Mode::Local => (2, Vec::new(), 0),
        };
        Value::Array(vec![
            Value::Uint32(mode),
            Value::Bytes(scope),
            Value::Uint32(maximum),
            Value::Uint64(self.response),
            stamp(self.sent),
            stamp(self.received),
        ])
    }
    // Reject malformed restored entitlements, timing and unknown private modes before any effect.
    pub(crate) fn parse(value: &Value) -> Result<Self, Error> {
        let Value::Array(v) = value else {
            return Err(Error::Invalid);
        };
        let [
            Value::Uint32(mode),
            Value::Bytes(scope),
            Value::Uint32(maximum),
            Value::Uint64(response),
            sent,
            received,
        ] = v.as_slice()
        else {
            return Err(Error::Invalid);
        };
        let mode = match mode {
            0 if scope.is_empty() && *maximum == 0 => Mode::Paid,
            1 if !scope.is_empty() && *maximum > 0 => Mode::Included {
                scope: Blob::new(scope.clone(), scope.len())?,
                maximum: *maximum,
            },
            _ => return Err(Error::Invalid),
        };
        if *response == 0 {
            return Err(Error::Invalid);
        }
        Ok(Self {
            mode,
            response: *response,
            sent: parse_stamp(sent)?,
            received: parse_stamp(received)?,
        })
    }
    // An included entitlement is never modeled as a zero-valued ordinary Ledger operation.
    pub(crate) fn included(&self) -> bool {
        matches!(self.mode, Mode::Included { .. })
    }
}
// Persist explicit clock domain with its bounded counter, not a process pointer or Instant.
fn stamp(stamp: Option<Stamp>) -> Value {
    Value::Array(
        stamp
            .into_iter()
            .map(|s| {
                Value::Array(vec![
                    Value::Bytes(s.epoch.to_vec()),
                    Value::Uint64(s.milliseconds),
                ])
            })
            .collect(),
    )
}
// Require exact optional clock tuples and reject changed widths on recovery.
fn parse_stamp(v: &Value) -> Result<Option<Stamp>, Error> {
    let Value::Array(values) = v else {
        return Err(Error::Invalid);
    };
    match values.as_slice() {
        [] => Ok(None),
        [Value::Array(v)] => {
            let [Value::Bytes(epoch), Value::Uint64(milliseconds)] = v.as_slice() else {
                return Err(Error::Invalid);
            };
            Ok(Some(Stamp {
                epoch: epoch.as_slice().try_into().map_err(|_| Error::Invalid)?,
                milliseconds: *milliseconds,
            }))
        }
        _ => Err(Error::Invalid),
    }
}
