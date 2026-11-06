# 0006: Board pseudocode accepts empty precondition names contrary to the specification

- Status: reported upstream
- Affected CR2SE module: Board
- Upstream revision: [d40b6278](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f)
- Upstream issue: https://github.com/beothorn/CR2SE/issues/9
- Resolution: pending; no implementation change made during planning.
- Duplicate search: open and closed repository issues searched for Board and the relevant duplicate/precondition terms on 2026-11-06; no matching report found.

## Evidence
Revision: d40b6278f24430e814d7fa82094f57261bfcac9f.

[Board Format](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/board/README.md#format) requires preconditions to be nonempty namespaced strings. [Pseudocode section 7](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/board/pseudocode/README.md#7-optional-standard-fields) checks only string type and explicitly says version 1 does not require nonempty precondition identifiers.

## Minimal reproduction
Take a valid fixed-price offering and matching definition, then set `preconditions:[""]`. validateOptionalPreconditions returns it successfully, whereas the normative field rule rejects it. A non-namespaced value such as `identity` also passes the helper without validating the namespace requirement.

## Impact and correction
The pseudocode and specification yield different structural validity in any implementation language. Require nonempty namespaced names, correct the explanatory paragraph without changing the permitted empty optional suggestion strings, and add acceptance cases. Clarify the namespace syntax if a precise grammar is intended; retain unknown but well-formed preconditions as unsupported required semantics rather than satisfied conditions.

## TalkSphere handling
Planning only: reject empty names according to the specification, keep unsupported semantics distinct from malformed syntax, and resolve namespace grammar against upstream before introducing a stricter local spelling rule.
