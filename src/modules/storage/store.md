# Storage upload and decision-time lease creation

Services supplies the original authorized invocation and complete logical body.
Stage checks exact dimensions, terms and price before allocating an ID or
reserving owner quota. It installs exact bytes after validating length and
SHA-256. It retains a proposal with all fixed lease fields and the purchased
duration. The proposal consumes reserved resources and establishes no active
lease. Confirmed staging failure releases the provisional quota. A possibly
committed write closes admission for recovery.

Template validation rederives the fixed fields from the original accepted
invocation. Finalize requires the exact prepared token and original template.
It takes an owner clock observation supplied at commit selection and produces
the complete result and effect. Expiration is checked addition. The coordinator
retains those exact bytes with its immutable decision. The Storage participant
compares the finalized output, effect and charge against the template before
applying the lease. Repeated commit delivery must match the original outcome.

The decision records both UTC and logical owner time. Reads, renewal, removal,
checks and expiry use elapsed owner time relative to that decision, preserving
the returned UTC timestamps across clock corrections.
