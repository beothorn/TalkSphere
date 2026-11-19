//! Services Requester callbacks retain exact original scope while delegating domain arithmetic and contextual result verification.
use super::{Client, input::roles};
use crate::modules::{
    board::{definition::Definition, offering::Offering},
    services::{
        Error,
        checks::Rules,
        handler::{Blob, Check, Invocation, Peer, Requester},
        pricing::Price,
        validation,
        value::{Budget, Value},
    },
    storage::{assessment, operation::Operation, terms::Terms},
};
impl Requester for Client {
    // Expose the exact independently recognized immutable definition installed with this code.
    fn definition(&self) -> &Definition {
        tracing::trace!("Borrowing independently installed Storage requester definition");
        &self.definition
    }
    // Economic understanding is independent of peer publication, reservations and existing lease obligations.
    fn terms(&self, offering: &Offering) -> Result<(), Error> {
        tracing::trace!("Validating current Storage requester offering economics");
        Terms::parse(offering).map(|_| ())
    }
    // Follow-up prices require the actual provider and attempt scope; issuer-only or ID-only lookup is never a fallback.
    fn pricing_for(&self, _: &Offering, _: &Value, _: &Value) -> Result<Price, Error> {
        tracing::trace!(
            "Refusing unscoped Storage quote pricing without an authenticated provider association"
        );
        Err(Error::Unsupported)
    }
    // Both local-provider and remote-provider quotes use the same exact original peer/key-aware arithmetic.
    fn pricing_with_peer(
        &self,
        peer: Peer,
        offering: &Offering,
        metadata: &Value,
        factors: &Value,
    ) -> Result<Price, Error> {
        tracing::trace!("Calculating exact scoped Storage requester quote");
        self.price(peer, offering, metadata, factors)
    }
    // A body-free projection contains independently known counts, not public provider-dependent pricingInputs.
    fn assessment_metadata(&self, input: &Value, budget: Budget) -> Result<Value, Error> {
        tracing::trace!("Deriving bounded Storage requester metadata without copying upload data");
        self.metadata(input, budget)
    }
    // Validate current offered store limits at projection while retaining the original peer association for later scoped quotes.
    fn assessment_metadata_with_peer(
        &self,
        peer: Peer,
        offering: &Offering,
        input: &Value,
        budget: Budget,
    ) -> Result<Value, Error> {
        tracing::trace!("Deriving Storage quote metadata under original authenticated selection");
        roles(peer)?;
        self.terms(offering)?;
        let projected = self.metadata(input, budget)?;
        if super::input::kind(input)? == Operation::Store {
            assessment::requester_metadata(input, offering, self.budget(budget))
        } else {
            Ok(projected)
        }
    }
    // No context-free remote-factor validation can infer the correct provider from an issuer or collision-prone ID alone.
    fn validate_assessment(&self, _: &Value, _: &Value, _: &Value, _: Budget) -> Result<(), Error> {
        tracing::trace!("Refusing unscoped Storage remote factor validation");
        Err(Error::Unsupported)
    }
    // Compare remote factors with independent original input and provider-scoped local expectations before any spending.
    fn validate_assessment_with_peer(
        &self,
        peer: Peer,
        offering: &Offering,
        input: &Value,
        metadata: &Value,
        factors: &Value,
        budget: Budget,
    ) -> Result<(), Error> {
        tracing::trace!(
            "Validating original Storage quote expectations against scoped remote factors"
        );
        let budget = self.budget(budget);
        metadata.bounded(budget)?;
        factors.bounded(budget)?;
        if self.assessment_metadata_with_peer(peer, offering, input, budget)? != *metadata {
            return Err(Error::Invalid);
        }
        self.price(peer, offering, metadata, factors).map(|_| ())
    }
    // Complete bodies must match original metadata and standard operation semantics; no placeholder body can pass.
    fn validate_input(&self, metadata: &Value, input: &Value, budget: Budget) -> Result<(), Error> {
        tracing::trace!(
            "Checking complete Storage requester input against original accepted projection"
        );
        let budget = self.budget(budget);
        validation::complete(
            input,
            &self.definition.document.value()["input"],
            budget,
            self,
        )?;
        metadata.bounded(budget)?;
        if self.metadata(input, budget)? != *metadata {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    // All ordinary results are independently checked under original invocation context before being accepted for settlement.
    fn validate_result(
        &self,
        invocation: &Invocation,
        input: &Value,
        output: &Value,
    ) -> Result<(), Error> {
        tracing::trace!("Checking complete scoped Storage requester result before settlement");
        self.result(invocation, input, output)
    }
    // Storage exposes only separately paid remote byte rules, never an included store allowance.
    fn checks(&self) -> Option<&dyn Rules> {
        tracing::trace!("Returning installed separately paid Storage byte-check rules");
        Some(self)
    }
    // The context-free postcommit port compares explicit original retrieval bytes and never decodes private accepted-journal bytes.
    fn verify(
        &self,
        input: &Value,
        _terms: &Blob,
        result: &Value,
        expectations: Option<&Value>,
    ) -> Result<Check, Error> {
        tracing::trace!(
            "Running Storage requester-local byte comparison without financial side effects"
        );
        self.compare_range(input, result, expectations)
    }
}
