//! Services adapter for the recovered Storage byte and metadata owners.
use super::{
    byte_check,
    contract::{Clock, Crypto},
    image::Stage,
    operation::{self, Operation},
    recovery::Gate,
    renewal, retrieve_result,
    terms::Terms,
};
use crate::modules::{
    board::offering::{Direction, Offering},
    services::{
        self, Error,
        contract::OperationKey,
        handler::{Assessment, Blob, Completion, Invocation, Peer, Status},
        input::Input,
        value::{Budget, Value},
    },
};
use std::sync::Arc;

/// Installed provider adapter. Startup recovery opens the gate before publication.
pub struct Provider {
    gate: Arc<Gate>,
    crypto: Arc<dyn Crypto>,
    clock: Arc<dyn Clock>,
    limits: retrieve_result::Limits,
    checks: byte_check::Admission,
}
impl Provider {
    /// Bind one Storage owner to explicit platform ports and bounded result/check policy.
    /// # Errors
    /// Invalid rejects zero check capacity or unusable byte and value limits.
    pub fn new(
        gate: Arc<Gate>,
        crypto: Arc<dyn Crypto>,
        clock: Arc<dyn Clock>,
        limits: retrieve_result::Limits,
        maximum_check_leases: usize,
    ) -> Result<Self, Error> {
        tracing::trace!("Binding the Storage provider to its recovered local owner");
        if limits.result_bytes == 0
            || limits.object_bytes == 0
            || limits.values.nodes == 0
            || limits.values.bytes == 0
            || limits.values.depth == 0
            || limits.values.depth > 31
        {
            return Err(Error::Invalid);
        }
        Ok(Self {
            gate,
            crypto,
            clock,
            limits,
            checks: byte_check::Admission::new(Default::default(), maximum_check_leases)?,
        })
    }

    // Normalize resolved input and keep assessment independent of accepted spending and execution.
    fn assess_value(
        &self,
        peer: Peer,
        offering: &Offering,
        input: &Input,
        budget: Budget,
    ) -> Result<Assessment, Error> {
        tracing::trace!("Assessing Storage against authoritative metadata before quoting");
        let budget = Budget {
            depth: budget.depth.min(self.limits.values.depth),
            bytes: budget.bytes.min(self.limits.values.bytes),
            nodes: budget.nodes.min(self.limits.values.nodes),
        };
        let (image, _) = self.gate.snapshot()?;
        if peer.local != image.provider || peer.remote != peer.key.requester {
            return Err(Error::Denied);
        }
        if is_store(input) {
            return super::assessment::admitted_store(
                &self.gate,
                peer.key,
                input,
                offering,
                budget,
                u64::MAX,
            )
            .map(|result| result.assessment);
        }
        if offering.direction != Direction::Provided {
            return Err(Error::Denied);
        }
        let value = input.complete(budget)?;
        let operation = operation::input(&value, budget)?;
        self.gate.admit(|journal, quota| {
            let image = journal.snapshot()?;
            let now = self.clock.now()?;
            let assessed = if operation == Operation::Renew {
                renewal::assess(
                    renewal::Context {
                        peer: &peer,
                        offering,
                        image: &image,
                        now,
                        reboot_elapsed: None,
                    },
                    input,
                    budget,
                    u64::MAX,
                )?
                .assessment
            } else {
                let id = renewal::id(&value)?;
                let lease = super::lease::authorize(
                    image.leases.iter().find(|lease| lease.id == id),
                    peer.local,
                    peer.remote,
                    &id,
                )?;
                if !super::retention::active(&image, lease, now, None)? {
                    return Err(Error::Unavailable);
                }
                if operation == Operation::Retrieve {
                    let offset = renewal::number(&value, "offset")?;
                    let length = renewal::number(&value, "length")?;
                    if length > Terms::parse(offering)?.maximum_retrieve_bytes {
                        return Err(Error::Limit);
                    }
                    if offset >= lease.size_bytes || length > lease.size_bytes - offset {
                        return Err(Error::Invalid);
                    }
                }
                Assessment {
                    input_bytes: u64::try_from(value.byte_len(budget)?)
                        .map_err(|_| Error::Limit)?,
                    metadata: value,
                    factors: Value::Object(vec![]),
                    pin: None,
                }
            };
            quota.probe(
                peer.key,
                super::quota::Usage {
                    operations: 1,
                    ..Default::default()
                },
            )?;
            Ok(assessed)
        })
    }
}

// Inspect only the fixed operation discriminator on unresolved descriptions.
fn is_store(input: &Input) -> bool {
    tracing::trace!("Checking Storage store discriminator without materializing an upload");
    let matches = |value: &Value| matches!(value, Value::String(name) if name == "store");
    match input {
        Input::Complete(Value::Object(fields)) => fields
            .iter()
            .any(|(name, value)| name == "operation" && matches(value)),
        Input::Object(fields) => fields.iter().any(|(name, value)| {
            name == "operation" && matches!(value, Input::Complete(value) if matches(value))
        }),
        _ => false,
    }
}

impl services::handler::Provider for Provider {
    // Reuse bounded metadata assessment for both complete and detached invocations.
    fn assess(&self, peer: Peer, offering: &Offering, input: &Value) -> Result<Assessment, Error> {
        tracing::trace!("Assessing a complete Storage request");
        self.assess_value(
            peer,
            offering,
            &Input::Complete(input.clone()),
            self.limits.values,
        )
    }

    // Store dimensions are available without reading optional detached byte fields.
    fn assess_metadata(
        &self,
        peer: Peer,
        offering: &Offering,
        input: &Input,
        budget: Budget,
    ) -> Result<Assessment, Error> {
        tracing::trace!("Assessing Storage metadata before body admission");
        self.assess_value(peer, offering, input, budget)
    }

    // Invoke exactly one accepted operation and retain its result through the independent owner.
    fn execute(&self, invocation: &Invocation, input: &Value) -> Result<Completion, Error> {
        tracing::trace!("Dispatching original accepted Storage work");
        let budget = self.limits.values;
        match operation::input(input, budget)? {
            Operation::Store => {
                super::store::stage(&self.gate, invocation, input, self.crypto.as_ref(), budget)
            }
            Operation::Renew => super::renewal_commit::stage(
                &self.gate,
                invocation,
                input,
                self.clock.now()?,
                None,
                budget,
            ),
            Operation::Remove => super::removal::stage(
                &self.gate,
                invocation,
                input,
                self.clock.now()?,
                None,
                budget,
            ),
            Operation::Retrieve => self.gate.admit(|journal, quota| {
                let image = journal.snapshot()?;
                let pin = super::retrieve::accept(
                    invocation,
                    input,
                    &image,
                    self.gate.byte_store(),
                    self.clock.now()?,
                    None,
                    budget,
                )?;
                quota.reserve(
                    invocation.key,
                    retrieve_result::reservation(&pin, self.limits)?,
                )?;
                let result = retrieve_result::stage(journal, &pin, self.limits);
                if result.as_ref().is_err_and(|error| *error != Error::Unknown) {
                    quota
                        .transition(invocation.key, Default::default())
                        .map_err(|_| Error::Unknown)?;
                }
                result
            }),
        }
    }

    // Recover the original provisional or retained result without doing work again.
    fn completion(&self, key: OperationKey) -> Result<Completion, Error> {
        tracing::trace!("Recovering original Storage completion");
        self.gate.completion(key)
    }

    // Deliver preparation against exactly the family-owned effect.
    fn prepare(&self, key: OperationKey, effect: &Blob) -> Result<Blob, Error> {
        tracing::trace!("Preparing the original Storage effect");
        self.gate.prepare(key, effect)
    }

    // Compare staged output against the original local effect, including finalization where applicable.
    fn validate_local_result(
        &self,
        invocation: &Invocation,
        input: &Value,
        result: &Completion,
    ) -> Result<bool, Error> {
        tracing::trace!("Validating Storage provider result against original local staged work");
        let original = self.gate.completion(invocation.key)?;
        if self.requires_finalization(invocation, input) {
            let (_, limits) = self.gate.snapshot()?;
            super::store::validate_template(invocation, input, &original, limits)?;
            super::store::validate_finalized(invocation.provider, limits, &original, result)?;
        } else if original.output != result.output
            || original.result != result.result
            || original.price != result.price
            || original.effect != result.effect
        {
            return Err(Error::Conflict);
        }
        Ok(true)
    }

    // Only store defines a result template with decision-assigned fields.
    fn requires_finalization(&self, _: &Invocation, input: &Value) -> bool {
        tracing::trace!("Selecting Storage decision-time result finalization");
        matches!(input, Value::Object(fields) if fields.iter().any(|(name, value)| name == "operation" && matches!(value, Value::String(op) if op == "store")))
    }

    // Recover fixed content and exact economics from authorized invocation data.
    fn validate_template(
        &self,
        invocation: &Invocation,
        input: &Value,
        template: &Completion,
    ) -> Result<(), Error> {
        tracing::trace!("Checking the original Storage creation template");
        let (_, limits) = self.gate.snapshot()?;
        super::store::validate_template(invocation, input, template, limits)
    }

    // Obtain the coherent wall and elapsed observation during coordinator decision selection.
    fn finalize(
        &self,
        key: OperationKey,
        token: &Blob,
        template: &Completion,
    ) -> Result<Completion, Error> {
        tracing::trace!("Sampling Storage lease time at commit selection");
        super::store::finalize(&self.gate, key, token, template, self.clock.now()?, None)
    }

    // Apply the exact coordinator decision, using its retained time across delayed delivery and restart.
    fn commit_finalized(
        &self,
        key: OperationKey,
        token: &Blob,
        template: &Completion,
        finalized: &Completion,
    ) -> Result<(), Error> {
        tracing::trace!("Committing a finalized Storage lease through its durable owner");
        self.gate.commit_store(key, token, template, finalized)
    }

    // Retrieval, renewal and removal preserve ordinary coordinator decision delivery.
    fn commit(&self, key: OperationKey, token: &Blob) -> Result<(), Error> {
        tracing::trace!("Committing an original Storage operation without finalization");
        self.gate.commit(key, token)
    }

    // Only the original explicit abort may release participant resources.
    fn abort(&self, key: OperationKey, token: Option<&Blob>) -> Result<(), Error> {
        tracing::trace!("Delivering an original Storage abort");
        if token.is_none() && self.gate.status(key)?.is_none() {
            return Ok(());
        }
        self.gate.abort(key, token)
    }

    // A staged proposal has no prepared token and therefore maps to Absent at the participant boundary.
    fn status(&self, key: OperationKey) -> Result<Status, Error> {
        tracing::trace!("Reading durable original Storage participant status");
        Ok(match self.gate.status(key)? {
            None | Some(Stage::Staged) => Status::Absent,
            Some(Stage::Prepared(token)) => Status::Prepared(token),
            Some(Stage::Committed(_)) => Status::Committed,
            Some(Stage::Aborted(_)) => Status::Aborted,
        })
    }

    // Produce only requester-authorized public evidence from the original retained byte.
    fn evidence(
        &self,
        invocation: &services::checks::Invocation,
        input: &Value,
    ) -> Result<Value, Error> {
        tracing::trace!("Reading one authorized Storage byte under accepted check terms");
        self.gate.admit(|journal, _| {
            let image = journal.snapshot()?;
            let now = self.clock.now()?;
            let mut epoch = [0; 32];
            epoch[..16].copy_from_slice(&now.boot);
            let stamp = services::checks::Stamp {
                epoch,
                milliseconds: now.elapsed_seconds.checked_mul(1000).ok_or(Error::Limit)?,
            };
            self.checks
                .accept(
                    byte_check::Context {
                        invocation,
                        image: &image,
                        bytes: self.gate.byte_store(),
                        now,
                        reboot_elapsed: None,
                        accepted: stamp,
                    },
                    input,
                    self.limits.values,
                )?
                .evidence()
        })
    }
}

impl super::requester::client::Leases for Provider {
    // Resolve only this identity's actual authoritative leases for local quote evaluation.
    fn lease(
        &self,
        key: OperationKey,
        provider: crate::modules::identity::api::IdentityId,
        requester: crate::modules::identity::api::IdentityId,
        id: [u8; 32],
    ) -> Result<super::contract::Lease, Error> {
        tracing::trace!("Reading provider-owned lease for original Storage pricing");
        let (image, _) = self.gate.snapshot()?;
        if image.provider != provider || key.requester != requester {
            return Err(Error::Denied);
        }
        super::lease::authorize(
            image.leases.iter().find(|lease| lease.id == id),
            provider,
            requester,
            &id,
        )
        .cloned()
    }
}

#[cfg(test)]
#[path = "tests/provider.rs"]
mod tests;
