# 0005: Board pseudocode permits duplicate-member policies that violate the specification

- Status: reported upstream
- Affected CR2SE module: Board
- Upstream revision: [d40b6278](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f)
- Upstream issue: https://github.com/beothorn/CR2SE/issues/8
- Resolution: pending; no implementation change made during planning.
- Duplicate search: open and closed repository issues searched for Board and the relevant duplicate/precondition terms on 2026-11-06; no matching report found.

## Evidence
Revision: d40b6278f24430e814d7fa82094f57261bfcac9f.

[Board Format](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/board/README.md#format) requires duplicate JSON members to be rejected. [BD-02](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/board/acceptanceTests/README.md) tests rejection. However, [pseudocode section 2](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/board/pseudocode/README.md#2-bounded-json-decoding) makes duplicate handling a configurable JSON-interface policy and only requires deterministic behavior; parseJsonWithLimits does not explicitly require rejection.

## Minimal reproduction
Decode `{"version":1,"providedServices":[],"wantedServices":[],"version":1}` with deterministic first-wins or last-wins parsing. Such parsing satisfies the pseudocode paragraph but violates the specification and BD-02. Escaped equivalent member names must also collide after JSON string decoding.

## Impact and correction
Implementations following the pseudocode can accept documents rejected by normative peers, in any language. Require duplicate-member rejection at every object depth in parseJsonWithLimits and replace the configurable-policy paragraph and corresponding minimum-case wording with mandatory rejection. Keep the old publication unchanged after failure.

## TalkSphere handling
Planning only: follow the normative rejection rule and add nested/escaped-name regression cases during implementation; no new wire behavior is proposed.
