# Storage requirements and integration audit

Behavior baseline: [CR2SE d40b6278](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f), rechecked 2026-11-17;
main has six later commits, none changing protocol files.
This is a requirements map, not executed acceptance evidence.

| Case | Required observable behavior | Owner / planned steps |
|---|---|---|
| ST-01 | Exact rational prices, one store ceiling, reject invalid maxima | Storage pricing; 4–6 |
| ST-02 | Accepted upload creates unique durable lease and one charge | Storage participant + Services/Ledger; 11–18, 32 |
| ST-03 | Bad/incomplete uploads never publish active leases or charge | Input validator + durable staging; 7, 16–18 |
| ST-04 | Authenticated owner only; foreign and missing look alike | Lookup + trusted invocation context; 8 |
| ST-05 | Exact positive ranges and whole-object hash | Provider/requester; 19–21 |
| ST-06 | Timely accepted reads survive expiry; new reads do not | Lease acceptance and byte pins; 9, 19–20 |
| ST-07 | Expected-expiry guard, serialized extension, checked bounds | Storage participant; 22–23, 33 |
| ST-08 | Logical terminal removal, no refund or erasure claim | Storage participant; 24–25 |
| ST-09 | New operations use current terms; existing obligation unchanged | Board selection + retained terms; 6, 28–29 |
| ST-10 | Private expected byte; local outcome; exact evidence charge | Checks + Services; 26–27; issue #10 limits global timeout claim |
| ST-11 | Owner/offset/timing and permitted frequency policy | Checks; 26–27 |
| ST-12 | Recovery once, no automatic remote retry, valid race history | Participant + Services; 13–15, 32–33 |
| ST-13 | Opaque ciphertext and genuinely independent providers | Storage + application integration; 35 |
| AT-C01 | Direct/serialized boundaries and unforgeable local authority | Public ports and integration; 2, 28–34 |
| AT-C02 | Finite budgets, exact widths, dependency uncertainty | All ports; 2–15, 32–33 |
| AT-C03 | Immutable snapshots/results survive restart | Byte handles and durable stores; 11–15, 32 |
| AT-C04 | Complete output only; no secrets or hidden optional calls | Validation/composition; 7, 18–21, 27–37 |
| SV-09 / AT-E04 | Every durable coordination cut recovers once | Actual Storage participant + independent Services/Ledger stores; 32 |
| AT-E02 | Wanted requester/payer independent of stream opener | Services/Peer Protocol + family; 30 |
| AT-E03 | Restore requester keys/catalog, recover from another provider | Application integration; 35 |
| AT-E06 | Independent issuer confirmation, exact fees, no work replay | Existing settlement contracts + Storage; 31 |

## Public port audit

Services Provider supplies assess/assess_metadata, execute, completion,
prepare/commit/abort/status and evidence. Invocation already carries authenticated
roles, accepted offering/definition, original quote and exact factors. Use these
public fields, never coordinator-private journal bytes. Requester supplies
assessment_metadata, validate_assessment/input/result, pricing, checks and verify.
Storage must override complete-value defaults for unresolved upload assessment.
Its cached_retry remains None; generic Storage has no remote resume/status API.

Services Blob and Value own bounded memory. Initial production Storage limits must
fit these contracts plus Peer Protocol's aggregate value/body/worker budgets.
Files do not turn Value into a streaming input. A future streaming improvement
requires explicit contract changes and cross-module regression tests.

Registry requires an independent Standards catalogue and complete definitions.
Board publication follows recovered handler availability. Local authority comes
from composition; peer payload fields never supply authenticated context.

Storage needs its own durable byte/metadata participant, immutable result handles,
finite quota accounting and recovered status. Existing Services/Ledger durable
stores remain independent. Recover coordinator decisions before serving affected
leases; an unavailable coordinator cannot imply abort. Read pin acceptance must
correspond to accepted operation terms, rather than a disposable assessment alone.
If the existing port timing cannot establish this boundary, introduce a narrow
explicit local contract with regression tests before claiming ST-06.

## Timing and uncertainty review

Lease acceptedAt must describe completed durable acceptance, not merely execute
entry. Coordinate staged expiry/time with the actual participant decision;
long preparation delays cannot silently consume promised initial retention.
Persist terminal transitions and protect accepted durations against wall-clock
correction. Across reboot a monotonic clock alone cannot measure offline elapsed
time: document the trusted UTC source and recovery assumptions, and investigate
contradictions before implementing dependent behavior.

Ordinary lost remote output may leave a durable lease and outcome_unknown.
Do not rerun execute or refund from connection loss. Existing
[issue #10](https://github.com/beothorn/CR2SE/issues/10) covers paid-check
postcommit response loss; preserve separate requester/provider accounting.
No additional confirmed protocol defect is asserted by this audit.

## Sources

[Specification](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md),
[inputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/inputs/README.md),
[outputs](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/outputs/README.md),
[pseudocode](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/pseudocode/README.md),
[ST-01–13](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/acceptanceTests/README.md),
[shared contracts](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/ModuleContracts.md).
