# 0007: Check timeout charging after provider commitment is ambiguous

- Status: reported upstream
- Affected CR2SE module(s): Services, Ledger, Storage/Messaging checks and Peer Protocol
- Upstream revision: [d40b6278f24430e814d7fa82094f57261bfcac9f](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f)
- Specification/contract/pseudocode/vector sections: [Services checks](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/services/README.md#checks), [SV-08/SV-09/SV-10](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/services/acceptanceTests/README.md), [durable coordination](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/ModuleContracts.md#durable-effects-across-process-boundaries), [Storage check](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md#10-storage-check), [peer result completion](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/README.md#input-result-and-failure)
- Upstream issue/fix: https://github.com/beothorn/CR2SE/issues/10

## Problem and evidence

The check rules describe a requester-local timeout without valid evidence as
uncharged, and SV-08 requires zero charge delta. The shared coordination rules
require a durable commit decision to survive response loss, prohibit rollback
from timeout, and explicitly distinguish local commitment from remote receipt.
The check rules do not specify whose balance delta is asserted or how to handle
a provider's already committed evidence charge when the response never reaches
the requester. The ordinary unknown-outcome rule does not explain whether or how
this check-specific zero-charge instruction overrides either node's accounting.

This is a distributed contract ambiguity in any implementation language; no
Rust ownership, crate, platform or persistence mechanism is involved.

## Minimal reproduction

1. A owns100 credits issued by B; B recognizes100 issued to A.
2. A and B accept a check priced3 B credits and reserve their local capacity.
3. B produces complete well-formed evidence, durably prepares and commits the
   local check/charge, reducing its recognized issued balance to97.
4. Lose the result before A receives complete valid evidence and normal end.
5. A's deadline expires. Applying the check timeout rule locally leaves its owned
   balance100 and releases its uncharged hold. Applying the shared commit rule
   leaves B's recognized balance97; B cannot undo a recorded commit from timeout.

An executable direct-contract trace is
[timeout_and_provider_commit_have_distinct_local_credit_views](https://github.com/beothorn/TalkSphere/blob/main/src/modules/services/tests/checks.rs).
Run `cargo test --locked timeout_and_provider_commit_have_distinct_local_credit_views`.
The test supplies two real accounting participants and injected peer delivery;
it does not claim live network interoperability.

## Expected behavior

Specify a consistent interpretation for both nodes, the applicability of
outcome-unknown, what a zero charge delta means, and what reconciliation (if any)
is authorized. A node must not silently assume that its local timeout reversed
an independently committed issuer balance.

## Actual or ambiguous behavior

A requester-only zero delta and an irreversible provider-side debit can both
follow the published instructions. The issuer may subsequently recognize fewer
spendable credits than the requester believes remain. The specification does
not distinguish this case from a check that failed before any provider commit.

## Impact

Honest implementations can disagree on check charging and available credits
after response loss, or an implementation may incorrectly erase a durable
obligation to satisfy the unqualified timeout assertion. Global uncharged-timeout
conformance cannot be inferred from a requester-only zero-delta test.

## Proposed protocol correction

Add a check-specific postcommit/lost-response case to SV-08 and distinguish
known uncommitted check failure from possible provider commitment. If the
zero-charge rule is deliberately only requester-local, state that explicitly,
retain the independent provider/issuer outcome and define the recovery limits.
If timeout must guarantee no recognized charge at either node, specify the
additional conditional settlement/receipt or reconciliation behavior instead of
implying that transport timeout provides atomic rollback. Preserve the distinction
between requester-computed check outcome and payment outcome.

## TalkSphere handling

The current direct-call implementation follows zero delta for requester-local
check timeout and never reverses a provider's recorded commit. The two-view test
records the resulting disagreement. This is an explicit scoped reading, not a
claim of globally uncharged checks or an invented wire handshake. Paid-check
network integration must revisit the upstream clarification; no automatic refund,
trust change or balance reconciliation is added.

## Resolution

Unresolved; main still has the reviewed protocol contracts. Open and closed issues
were searched before filing; none covered this postcommit check-timeout case.
