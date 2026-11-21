# 0009: Unavailable authorized Storage byte has no distinct wire report

- Status: resolved upstream
- Affected CR2SE module(s): Storage, Peer Protocol failure codes
- Upstream revision: [24cce05bb258211b1fe07914bcd0a7743cfcd91c](https://github.com/beothorn/CR2SE/tree/24cce05bb258211b1fe07914bcd0a7743cfcd91c)
- Specification/contract/pseudocode/vector sections: [Storage check](https://github.com/beothorn/CR2SE/blob/24cce05/modules/storage/README.md#10-storage-check), [Peer Protocol failure codes](https://github.com/beothorn/CR2SE/blob/24cce05/modules/peer-protocol/README.md#input-result-and-failure)
- Upstream issue/fix: https://github.com/beothorn/CR2SE/issues/12

## Problem and evidence

The Storage check outcome treats an authenticated report that the active,
authorized byte is unavailable as `fail`. The Peer Protocol failure table only
offers `not_available`, which also covers inactive, missing or concealed
authorization refusals, and `internal_error`, a generic execution failure.
A requester cannot tell a real storage failure from conditions that establish
nothing about the stored data. No language or platform assumption is involved.

## Expected behavior

An authenticated owner of an active lease, after exact offset validation, can
receive a distinct, uncharged, challenge-bound report only when the stored byte
cannot be returned.

## Actual or ambiguous behavior

Both a real loss and an unrelated refusal use the same wire codes, so the
specified `fail` outcome is unreachable without trusting an ambiguous code.

## Impact

Implementations either never record a failed check or record one for refusals
that prove nothing, producing divergent trust and accounting behavior.

## Proposed protocol correction

Define a distinct failure code usable only for that condition, and require a
complete timely failure with normal END before the requester uses it. Generic
rejection, timing ambiguity and unauthorized access stay inconclusive.

## TalkSphere handling

The Peer Protocol and Services layers carry the new report as
`storage_byte_unavailable`. A timely authenticated report for a validated
active-lease challenge aborts both uncommitted holds and lets an eligible
requester record a failed check; generic refusal and lost evidence remain
inconclusive. Tests are in the Storage provider and byte-check suites.

## Resolution

[Issue #12](https://github.com/beothorn/CR2SE/issues/12) is closed by
[CR2SE bc53df4](https://github.com/beothorn/CR2SE/commit/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16),
which defines the distinct report.
