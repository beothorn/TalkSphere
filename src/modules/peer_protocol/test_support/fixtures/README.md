# Independent Peer Protocol fixtures

Control/binary bytes and the logical value examples are transcribed from
[upstream vectors](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/peer-protocol/pseudocode/TestVectors.md),
file Git blob43b66be73740988d42a27ae5c1f5fa3919196238.
CONTROL includes exactly20 body bytes, without LF. BODY declares ID/offset0
with ASCII abc. Values retain the displayed canonical JSON forms and negative
spellings. The whitespace-bearing quote is a local hash-association fixture,
not a complete wire-valid service quote; quote.sha256 was calculated using Python
hashlib over its exact original bytes. Malformed record mutations are derived in
individual parser tests. These are application bytes inside DATA, distinct from
Transport authentication vectors and the Services local binary adapter.
