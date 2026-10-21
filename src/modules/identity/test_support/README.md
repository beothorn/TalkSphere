# Identity deterministic helpers

These cfg(test)-only helpers supply ordered exact crypto calls, scripted entropy,
protected in-memory records with restart/failure behavior and contact snapshots.
Boolean authorities are test substitutes, not production credentials. Real secret
cleanup observation remains available through Cryptography's test-only probes.

Fixtures are verbatim CR2SE files from revision
[d40b6278f24430e814d7fa82094f57261bfcac9f](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f):
Transport TestVectors.md, Cryptography pseudocode/README.md and Cryptography
acceptanceTests/README.md. Their embedded relative links refer to upstream.
Private values are published test fixtures and must never become production keys.

From the root, build with `cargo build --locked`, exercise helpers with
`cargo test --locked modules::identity::test_support`, and view runtime help with
`cargo run --locked -- --help`. Helpers open no listeners and make no network calls.
