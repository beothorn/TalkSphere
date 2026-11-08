# Coordinator admission

These operations use the shared [coordinator contract](mod.md). Admit retains exact consent before one original-key reservation; supply_input retains only complete validated input.

Every call requires the matching owner-issued Context and rechecks version, deadline, cancellation, authority and journal consistency under serialization. Missing bindings, invalid phase, conflicting reuse, bounded capacity, dependency/storage failure and unknown outcomes stay typed. State and results outlive client buffers; only exact installed public participant contracts are used. See coordinator documentation for each method’s inputs, outputs, action and failure rules.
