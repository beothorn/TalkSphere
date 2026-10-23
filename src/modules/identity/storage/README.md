# Protected local Identity storage

The Linux adapter uses owner-only directories (0700), regular files (0600),
anchored no-follow access and an exclusive writer lock. It supplies local platform
persistence, separate from paid Storage. It protects against other users, not
root or another process under the same account; records are not encrypted.

Status: protected reads are implemented; atomic key installation is next.
[Contracts](file.md) describe authority, limits and failures. The platform's
responsibility is safe durable storage; Identity validates key associations and
Cryptography handles private operations. No secret is returned in diagnostics.

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::identity::storage
cargo run --locked -- --help
cargo run --locked -- help serve
```

Tests use private temporary directories and fixture keys. There is no storage CLI
command yet and serve remains unavailable. Platforms other than Linux explicitly
return Unsupported until a protected adapter is implemented and tested. The
[Identity plan](../../../../plans/3_Identity_module/PLAN.md) records the pinned
CR2SE baseline; file protection and serialization are local implementation choices.
