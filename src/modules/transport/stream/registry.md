# Protocol registry

Trusted composition calls `Registry.register` with a protocol identifier and
version. Invalid OPEN selections return `RegistrationError.Invalid`; an existing
exact registration returns `Duplicate`. Success issues a new `StreamOwner`.
A peer cannot create registrations. `resolve` maps a validated selection to its
owner or returns no match; `protocol` maps an owner capability back to its
registered selection. Versions are matched exactly and no negotiation occurs.
