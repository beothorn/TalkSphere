# Coordinator inspection

These operations use the shared [coordinator contract](mod.md). Result returns owned complete output only after local commitment; verify uses private caller expectations without changes to credits/trust; status observes retained phase without payload disclosure.

Every call requires the matching owner-issued Context and rechecks version, deadline, cancellation, authority and journal consistency under serialization. Missing bindings, invalid phase, conflicting reuse, bounded capacity, dependency/storage failure and unknown outcomes stay typed. State and results outlive client buffers; only exact installed public participant contracts are used. See coordinator documentation for each method’s inputs, outputs, action and failure rules.

completed returns both the original immutable Invocation and retained Completion
under Observe authority, only after the same fully acknowledged commitment
required by result. The wire adapter can therefore validate original roles,
contract, factors and output without reading coordinator internals or accepting a
caller-fabricated completion. Checks remain on their separate evidence port.
