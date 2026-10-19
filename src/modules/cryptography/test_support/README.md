# Cryptography test support

This cfg(test)-only module supplies scripted exact-length entropy with explicit
errors/exhaustion, release counters and literal fixture decoding. It is not a
production randomness adapter or authenticated provider. Release counters observe
ownership events, not physical erasure. Run from the repository root with
`cargo test --locked modules::cryptography::test_support`; build with
`cargo build --locked` and inspect application help with `cargo run --locked -- --help`.

Both fixtures are pinned to CR2SE revision
[80f6795864ecf060e23482ee42909da4a649ecf0](https://github.com/beothorn/CR2SE/tree/80f6795864ecf060e23482ee42909da4a649ecf0).
The envelope file includes its source; the Transport file is copied from
[Transport vectors](https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/transport/pseudocode/TestVectors.md).
Private fixture values are public test material, never usable production credentials.
