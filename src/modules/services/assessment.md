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
