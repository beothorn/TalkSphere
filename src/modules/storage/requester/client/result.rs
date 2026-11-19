//! Contextual result validation happens before requester settlement and never mutates the application recovery catalogue.
use super::{
    Client,
    input::{field, kind, number},
};
use crate::modules::{
    services::{
        Error,
        handler::{Check, Invocation},
        validation,
        value::Value,
    },
    storage::{
        operation::{self, Operation},
        requester::{remove, renew, retrieve, store},
    },
};
impl Client {
    // Validate complete original input/output and independently retained lease/byte expectations before accepting financial work.
    pub(super) fn result(
        &self,
        invocation: &Invocation,
        input: &Value,
        output: &Value,
    ) -> Result<(), Error> {
        tracing::trace!(
            "Validating original accepted Storage result through installed requester code"
        );
        if !self.definition.same_contract(&invocation.definition) {
            return Err(Error::Invalid);
        }
        let definition = self.definition.document.value();
        validation::complete(input, &definition["input"], self.policy.values, self)?;
        validation::complete(output, &definition["output"], self.policy.values, self)?;
        let operation = operation::input(input, self.policy.values)?;
        if operation == Operation::Store {
            let observed = self.clock.now()?.unix_seconds;
            store::validate(
                invocation,
                input,
                output,
                self.policy.values,
                store::Policy {
                    observed_unix_seconds: observed,
                    maximum_clock_difference_seconds: self.policy.maximum_clock_difference_seconds,
                },
            )?;
            return Ok(());
        }
        let lease = self.original(
            invocation.key,
            invocation.provider,
            invocation.requester,
            crate::modules::storage::renewal::id(input)?,
        )?;
        match operation {
            Operation::Store => Err(Error::Invalid),
            Operation::Retrieve => {
                // Reject invalid output/count/range before a local expectation port can read or allocate its requested bytes.
                retrieve::validate(invocation, input, output, &lease, None, self.policy.values)?;
                let expected = self.leases.range(
                    invocation.key,
                    invocation.provider,
                    lease.id,
                    number(input, "offset")?,
                    number(input, "length")?,
                )?;
                if let Some(expected) = expected {
                    retrieve::validate(
                        invocation,
                        input,
                        output,
                        &lease,
                        Some(expected.bytes()),
                        self.policy.values,
                    )?;
                }
                Ok(())
            }
            Operation::Renew => {
                renew::validate(invocation, input, output, &lease, self.policy.values).map(|_| ())
            }
            Operation::Remove => {
                remove::validate(invocation, input, output, &lease, self.policy.values).map(|_| ())
            }
        }
    }
    // Postcommit context-free local checks compare only explicitly supplied original retrieval bytes; other conclusions need scoped validators.
    pub(super) fn compare_range(
        &self,
        input: &Value,
        output: &Value,
        expected: Option<&Value>,
    ) -> Result<Check, Error> {
        tracing::trace!(
            "Comparing locally observed Storage retrieval bytes without changing payments or trust"
        );
        operation::input(input, self.policy.values)?;
        if kind(input)? != Operation::Retrieve {
            return Ok(Check::Inconclusive);
        }
        operation::output(output, Operation::Retrieve, self.policy.values)?;
        let Some(Value::Bytes(expected)) = expected else {
            return Ok(Check::Inconclusive);
        };
        if expected.len() > self.policy.values.bytes {
            return Err(Error::Limit);
        }
        if field(input, "leaseId")? != field(output, "leaseId")?
            || number(input, "offset")? != number(output, "offset")?
        {
            return Ok(Check::Fail);
        }
        let Value::Bytes(data) = field(output, "data")? else {
            return Err(Error::Invalid);
        };
        if u64::try_from(expected.len()).map_err(|_| Error::Limit)? != number(input, "length")? {
            return Ok(Check::Inconclusive);
        }
        Ok(if data == expected {
            Check::Pass
        } else {
            Check::Fail
        })
    }
}
