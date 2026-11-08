# Family contracts

Requester supplies a complete installed Definition, validates economic terms and
calculates local Check outcomes (Pass, Fail, Inconclusive) from input, accepted
terms, result and privately held expectations. It also implements semantic value
validation. Absence of expectations cannot establish Pass.

Provider supplies assessment, execute, completion, prepare, commit, abort, status
and evidence. Assessment has metadata, price factors, input byte limit and an
optional owner-retained pin. It performs no work or settlement. Peer carries
local/authenticated remote identities and original operation key from composition.
Invocation copies that key, requester/provider roles, accepted bytes and maximum.

Execution only follows reservation and complete validation. Completion contains
logical output, retained result bytes, final price and an opaque staged effect
reference. Recovery calls completion and status, never repeats execution after
uncertainty. Prepare returns a durable owner/key/effect-bound token. Commit and
abort require the coordinator's immutable recorded decision. Prepared effects
cannot time out into abort; duplicate exact decisions are idempotent. Status is
Absent, Prepared(token), Committed or Aborted. Evidence serves only defined remote
checks after separate entitlement/payment authorization, never provider-calculated
check conclusions. Unsupported local-only checks return Unsupported.

Blob retains immutable complete bytes independently of sessions and limits input
length before ownership conversion. bytes borrows them while an owner remains.
The reference contains no filesystem path. It does not itself establish durability;
the coordinator or participant must persist bytes before acknowledging durable work.

All ports return typed Services errors; Unknown preserves possible commitment.
Handlers own family resources and cannot inspect Board/Ledger tables or open sockets
through these interfaces. Application composition authorizes caller channels.

Requester.pricing calculates exact Price from validated offering terms and provider
factors. The default checks terms and supports only positive fixed prices; custom
calculated models must explicitly override it. Computation metering remains subject
to the exact service/version eligibility rules in the shared pricing validator.

Requester.validate_input independently validates complete typed input against the
accepted metadata and finite Budget. The conservative default requires identical
metadata/input after complete schema/semantic validation. Body-bearing families
must explicitly validate their own length/hash/content relations in an override;
descriptions cannot relax this requirement.

Invocation exposes publisher, issuer, complete public Offering/Definition, original
quote, accepted pricing and factors explicitly. Handlers must not decode Services'
private journal/binding format to reconstruct their contracts. These copies remain
immutable at execution even if the Board or registry changes.

Requester.validate_result checks immutable quote-dependent sizes and contextual
operation rules after Services independently validates output schema/semantics.
The default has no additional contextual restriction. Installed standard families
must implement their specified validation; this does not turn optional families
into runtime dependencies or accept arbitrary descriptions as code.

Requester.validate_assessment independently checks remote metadata and factors
against the authorized local input source. Descriptions or remote offers cannot
replace that source. The default requires exact input/metadata equality, known
schema semantics and empty factors; family overrides remain trusted installed code.
