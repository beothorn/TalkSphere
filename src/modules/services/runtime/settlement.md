# Coordinator settlement

These operations use the shared [coordinator contract](mod.md). Prepare obtains exact participant tokens; decide persists one immutable outcome; commit/abort deliver only that decision and retain uncertainty until acknowledgements.

Every call requires the matching owner-issued Context and rechecks version, deadline, cancellation, authority and journal consistency under serialization. Missing bindings, invalid phase, conflicting reuse, bounded capacity, dependency/storage failure and unknown outcomes stay typed. State and results outlive client buffers; only exact installed public participant contracts are used. See coordinator documentation for each method’s inputs, outputs, action and failure rules.
