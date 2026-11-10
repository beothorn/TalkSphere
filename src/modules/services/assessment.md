# Bounded provider assessment

assess consumes Authorized selected terms, bounded metadata, a caller Context,
receiving Authority and finite Limits. Only the selected local provider can
assess; same-identity work requires explicit requester authority. Metadata is
bounded before calling the pinned provider and checked against its byte grant.
The provider validates operation semantics, authorization, capacity and metadata.

Returned metadata, factors and optional pin must fit finite byte/node/depth
budgets. The input length bound cannot exceed local capacity. Caller authority
and deadline are checked again after provider assessment. Refusal does not
execute, reserve money or admit body bytes. Assessed owns the authorization and
assessment; factors, input_bytes and authorized expose immutable observations.
Dropping it releases provisional pins. Execution and quote acceptance remain
separate lifecycle stages.

remote validates an explicitly authorized local requester's remote provider
assessment without invoking a local provider. It rejects local pins in remote
data, bounds metadata/factors/input length, checks Request authority and calls the
installed requester to validate factors against its own authorized input. The
default permits matching complete metadata and no provider-dependent factors;
body-bearing or calculated families must supply their normative validation.

Assess_metadata takes the same owner authority/context and Authorized binding,
plus an Input description and finite Limits. It independently checks declared
logical bytes, nodes, depth and body count before calling Provider.assess_metadata.
The default provider accepts only complete values; body-bearing handlers must
implement their actual metadata rules without invented bytes. Returned metadata,
factors and optional provisional pin are bounded and authority is rechecked before
retention. Existing assess still handles complete Value input. Neither operation
reserves Ledger funds or executes work.
