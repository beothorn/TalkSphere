//! Installed test-owned byte-evidence family; expectations never enter its provider port.
use super::family::Echo;
use crate::modules::{
    board::{definition::Definition, offering::Offering},
    services::{
        Error,
        checks::{self, Mode, Timing},
        contract::OperationKey,
        handler::*,
        validation::Semantics,
        value::Value,
    },
};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
pub(in crate::modules::services) struct Bytes {
    definition: Definition,
    echo: Echo,
    included: bool,
    local_only: bool,
    pub(in crate::modules::services) calls: AtomicUsize,
    pub(in crate::modules::services) seen: Mutex<Vec<Value>>,
}
impl Bytes {
    // A local-only installed check never has a provider-evidence invocation.
    pub(in crate::modules::services) fn local_only() -> Self {
        let mut family = Self::new(false);
        family.local_only = true;
        family
    }
    // The descriptor explicitly distinguishes provider evidence from requester-calculated outcomes.
    pub(in crate::modules::services) fn new(included: bool) -> Self {
        let echo = Echo::new();
        let mut definition = echo.definition().clone();
        let mut v = definition.document.value().clone();
        v["check"] = serde_json::json!({"description":"Challenge the pre-existing test resource at one byte offset; compare the one-byte evidence with a privately held byte; missing evidence is inconclusive, and no expected byte is transmitted.","input":{"type":"uint64"},"output":{"type":"object","fields":{"outcome":{"type":"string","description":"Requester-computed pass, fail or inconclusive."},"value":{"type":"bytes","description":"Complete provider evidence when present.","optional":true}}}});
        definition.document = crate::modules::board::document::Document::parse(
            v.to_string().as_bytes(),
            Default::default(),
        )
        .unwrap();
        Self {
            definition,
            echo,
            included,
            local_only: false,
            calls: AtomicUsize::new(0),
            seen: Mutex::new(Vec::new()),
        }
    }
}
impl checks::Rules for Bytes {
    // The included fixture represents one already-paid existing placement with a stable owner reference.
    fn mode(&self, _: &Offering, _: &Value) -> Result<Mode, Error> {
        Ok(if self.local_only {
            Mode::Local
        } else if self.included {
            Mode::Included {
                scope: Blob::new(vec![42; 32], 32)?,
                maximum: 1,
            }
        } else {
            Mode::Paid
        })
    }
    // The literal fixture contract fixes one second for evidence receipt.
    fn response_milliseconds(&self, _: &Offering) -> Result<u64, Error> {
        Ok(1000)
    }
    // Only an exact uint64 offset into the fixed five-byte pre-existing resource is supported.
    fn shape(&self, c: &Value) -> Result<(), Error> {
        if matches!(c,Value::Uint64(n) if *n<5) {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }
    // Authentication and active-resource authority are supplied explicitly by the owner context.
    fn challenge(&self, _: &checks::Invocation, c: &Value) -> Result<(), Error> {
        self.shape(c)
    }
    // Wrong but complete bytes remain valid billable evidence; malformed lengths cannot charge.
    fn evidence(&self, c: &Value, e: &Value) -> Result<(), Error> {
        self.shape(c)?;
        if matches!(e,Value::Bytes(b) if b.len()==1) {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }
    // Only the requester-held byte and locally comparable timing determine the property result.
    fn outcome(
        &self,
        _: &Value,
        e: Option<&Value>,
        expected: Option<&Value>,
        timing: Timing,
    ) -> Check {
        match (e, expected, timing) {
            (Some(e), Some(expected), Timing::Within(_)) if e == expected => Check::Pass,
            (Some(_), Some(_), Timing::Within(_)) => Check::Fail,
            _ => Check::Inconclusive,
        }
    }
}
impl Semantics for Bytes {
    // Ordinary work retains echo's independent text semantics.
    fn validate(&self, s: &serde_json::Value, v: &Value) -> Result<(), Error> {
        self.echo.validate(s, v)
    }
    // No fixture arithmetic permits nonfinite logical values.
    fn nonfinite(&self) -> bool {
        false
    }
}
impl Requester for Bytes {
    // The separately retrieved definition names the actual remote check procedure.
    fn definition(&self) -> &Definition {
        &self.definition
    }
    // Fixed ordinary work remains7; separately paid evidence must explicitly quote3.
    fn terms(&self, o: &Offering) -> Result<(), Error> {
        self.echo.terms(o)
    }
    // Remote evidence support is explicit installed code, not inferred from text.
    fn checks(&self) -> Option<&dyn checks::Rules> {
        Some(self)
    }
    // Ordinary local verification remains independent of remote evidence dispatch.
    fn verify(&self, i: &Value, t: &Blob, r: &Value, e: Option<&Value>) -> Result<Check, Error> {
        self.echo.verify(i, t, r, e)
    }
}
impl Provider for Bytes {
    // Ordinary assessment cannot invoke an evidence check or inspect expected values.
    fn assess(&self, p: Peer, o: &Offering, v: &Value) -> Result<Assessment, Error> {
        self.echo.assess(p, o, v)
    }
    // Ordinary work still follows the same staged participant ports.
    fn execute(&self, i: &Invocation, v: &Value) -> Result<Completion, Error> {
        self.echo.execute(i, v)
    }
    // Recovery retrieves an existing ordinary completion without executing.
    fn completion(&self, k: OperationKey) -> Result<Completion, Error> {
        self.echo.completion(k)
    }
    // Ordinary preparation preserves exact owner/key/effect binding.
    fn prepare(&self, k: OperationKey, e: &Blob) -> Result<Blob, Error> {
        self.echo.prepare(k, e)
    }
    // Ordinary commit is idempotent under the recorded decision.
    fn commit(&self, k: OperationKey, t: &Blob) -> Result<(), Error> {
        self.echo.commit(k, t)
    }
    // Ordinary abort cannot undo committed work.
    fn abort(&self, k: OperationKey, t: Option<&Blob>) -> Result<(), Error> {
        self.echo.abort(k, t)
    }
    // Status is a participant observation, never a provider correctness claim.
    fn status(&self, k: OperationKey) -> Result<Status, Error> {
        self.echo.status(k)
    }
    // The provider sees only the offset challenge and emits one byte; expected bytes are absent from this interface.
    fn evidence(&self, i: &checks::Invocation, c: &Value) -> Result<Value, Error> {
        checks::Rules::challenge(self, i, c)?;
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen.lock().unwrap().push(c.clone());
        Ok(Value::Bytes(vec![b'i']))
    }
}
