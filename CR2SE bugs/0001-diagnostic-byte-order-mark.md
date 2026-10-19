# 0001: Diagnostic payload pseudocode accepts a leading byte-order mark

- Status: resolved
- Affected CR2SE module(s): Transport
- Upstream revision: [8e848eb2e70ef7cd4490a753f9e647fdddcd99fd](https://github.com/beothorn/CR2SE/tree/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd)
- Specification/pseudocode: [wire section 2](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/README.md#2-integer-and-text-encoding),
  [wire sections 10–11](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/README.md#11-error-and-cancellation-payloads),
  [payload pseudocode section 10](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/pseudocode/README.md#10-payload-codecs)
- Upstream fix: [80f6795](https://github.com/beothorn/CR2SE/commit/80f6795864ecf060e23482ee42909da4a649ecf0)

## Problem and evidence

Wire section 2 requires text to be valid UTF-8 without a byte-order mark unless
a field explicitly says otherwise. Diagnostic reasons have no such exception.
The shared `parseErrorPayload` pseudocode checks UTF-8 validity, length and code,
but never rejects a leading BOM. `encodeErrorPayload` and `encodeGoAwayPayload`
also have no corresponding restriction on their input reason. This is a
language-independent missing validation rule: EF BB BF is valid UTF-8.

## Minimal reproduction

Supply an ERROR/CANCEL payload with these hexadecimal bytes:

```text
00 01 00 03 EF BB BF
```

The code is PROTOCOL_ERROR, the reason length is three, and the reason consists
only of a UTF-8 BOM. Every stated `parseErrorPayload` check succeeds. Prefixing
four zero bytes produces a GOAWAY payload that passes the same parser. Conversely,
encoding the reason consisting of U+FEFF passes the stated encoder preconditions.
This trace evaluates the published pseudocode; it is not a claim that an upstream
executable was run.

## Expected behavior

Reject the diagnostic for violating the BOM restriction. An outgoing codec
must not emit a reason beginning with a BOM either.

## Actual or ambiguous behavior

The pseudocode accepts the reason and returns it as valid, whereas the normative
text rule forbids it. Generic UTF-8 validity alone cannot enforce that rule.

## Impact

Implementations following the pseudocode alone can emit or accept payloads
rejected by implementations following the wire specification, causing avoidable
connection failures and inconsistent validation across programming languages.

## Proposed protocol correction

After checking valid UTF-8, explicitly reject a reason whose initial bytes are
EF BB BF. Add the same condition to both encoders. Preserve other valid UTF-8
without normalization; this is a restriction on a leading BOM, not a blanket
ban on the same code point inside text. Add these cases to TR-13.

## TalkSphere handling

TalkSphere follows the normative text rule rather than the incomplete pseudocode;
this is not a new wire rule or a temporary protocol extension. Its
[regression test](../src/modules/transport/codec/control/tests.rs) checks incoming
and outgoing ERROR/CANCEL and GOAWAY, plus preservation of interior characters:

```sh
cargo test --locked leading_byte_order_marks_are_rejected_but_interior_characters_are_preserved
```

## Resolution

Verified by source review at [80f6795](https://github.com/beothorn/CR2SE/commit/80f6795864ecf060e23482ee42909da4a649ecf0).

The diagnostic parser and both encoders now reject leading EF BB BF bytes; upstream acceptance cases and vectors cover rejection and preservation of interior U+FEFF.

The evidence above describes the original affected revision. This resolution
records the upstream correction, not a new full Transport conformance run.
