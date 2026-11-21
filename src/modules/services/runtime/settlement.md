# Coordinator settlement

These operations use the shared [coordinator contract](mod.md). Prepare obtains exact participant tokens; decide persists one immutable outcome; commit/abort deliver only that decision and retain uncertainty until acknowledgements.

Every call requires the matching owner-issued Context and rechecks version, deadline, cancellation, authority and journal consistency under serialization. Missing bindings, invalid phase, conflicting reuse, bounded capacity, dependency/storage failure and unknown outcomes stay typed. State and results outlive client buffers; only exact installed public participant contracts are used. See coordinator documentation for each method’s inputs, outputs, action and failure rules.

Template work uses the original result/effect reference for Ledger preparation
and settlement. After preparation, decision selection calls the Provider to
fill the template and checks the complete result and exact price. The commit
record retains the template and finalized completion together. Delivery
passes that recorded result to the Provider and successful inspection exposes
the finalized output. If a decision write was uncertain, reopen the journal
before selecting again. Recovering a recorded commit requires no resampling.
An Invalid or Limit finalization records abort for ordinary abort delivery.
