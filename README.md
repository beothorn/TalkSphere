# TalkSphere

TalkSphere is a Rust server implementation and proof of concept for
[CR2SE](https://github.com/beothorn/CR2SE). It will grow module by module into a
node that exchanges services and storage, followed by messaging, public boards,
pages and the other CR2SE capabilities. See the [implementation plan](plans/PLAN.md),
the [Transport breakdown](plans/1_Transport_module/PLAN.md) and the
[Cryptography breakdown](plans/2_Cryptography_module/PLAN.md). The next module has
an [Identity plan](plans/3_Identity_module/PLAN.md) ready for review; implementation
has not started.

The intended model is ad-free social features supported by shared device
storage and paid backup space. Compatibility with CR2SE clients is an
implementation goal.  

This implementation is focused on being a server (always running), but will also have client 
features for testing and for advanced users. Better clients with ui that are not using the cli must
be implemented separetely.

**Current status:** Transport and Cryptography's planned implementations are complete.
Real primitives verify both Transport handshake roles and fixed protected-frame
vectors; generic recipient encryption authenticates complete bounded objects.
Identity persistence and the authenticated two-node demonstration remain
[roadmap steps 4–5](plans/PLAN.md); `serve` reports the missing integration and
opens no listener. See the [Transport evidence](src/modules/transport/ACCEPTANCE.md)
and [Cryptography acceptance matrix](src/modules/cryptography/ACCEPTANCE.md) for
verified behavior, scope and unresolved protocol findings.

The [Cryptography guide](src/modules/cryptography/README.md) describes available
operations and verified vectors. Exercise it with
`cargo test --locked modules::cryptography`.

## Build and test

Install a current stable Rust toolchain with Cargo, rustfmt and Clippy. The
project uses Rust edition 2024, `tracing` for structured diagnostics and Mio for
nonblocking TCP readiness.
Run these commands from the repository root:

```sh
cargo build --locked
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
```

Run only the deterministic Transport helpers with
`cargo test --locked modules::transport::test_support`; their
[README](src/modules/transport/test_support/README.md) describes scripted I/O,
manual time, dependency failures and pinned fixtures.

Exercise frame header and control payload encoding/validation with
`cargo test --locked modules::transport::codec`; see the
[codec guide](src/modules/transport/codec/README.md) for its API and scope.
Run `cargo test --locked --test control_payloads` for public API composition
and trace privacy tests. See [production coverage](docs/coverage.md) for the
coverage setup, exclusions and measurement procedure.

Cargo.lock is tracked for reproducible application builds. For an optimized
build, run `cargo build --locked --release`; the executable is
`target/release/talksphere` (`talksphere.exe` on Windows).

## Run and get help

```sh
cargo run --locked -- --help
cargo run --locked -- help serve
cargo run --locked -- help --help
cargo run --locked -- help --version
cargo run --locked -- --version
cargo run --locked -- serve
```

The last command currently exits with status **1**, explaining that Identity
and normal server composition are still required. It opens no sockets. With no arguments, the application
shows its overview and exits successfully. Invalid arguments exit with status
**2** and a usage hint.

`--help` (or `-h`) lists all commands, options and detailed help topics. Use
`help <topic>` for details on each option or command; option spellings such as
`help --version` work too. `serve --help` also shows the serve topic.
`--version` (or `-V`) reports the application version, not CR2SE conformance.
After building, replace `cargo run --locked --` with
`./target/debug/talksphere` to run the executable directly.

## Project layout

```text
src/
  main.rs                  executable entry point
  lib.rs                   module exports
  cli/                     argument parsing and shared help definitions
  composition/             real Cryptography adapter for Transport
  modules/
    cryptography/          primitives, recipient envelopes, provider and tests
    transport/             protocol state, control driver, TCP adapters and tests
tests/                     executable integration tests
CR2SE bugs/                language-independent protocol bug reports
AGENTS.md                  instructions for ongoing implementation
plans/
  PLAN.md                  staged implementation checklist
  1_Transport_module/
    PLAN.md                Transport steps and matching detailed instructions
  2_Cryptography_module/
    PLAN.md                Cryptography steps and matching details
  3_Identity_module/
    PLAN.md                proposed Identity steps and matching details
```

Protocol modules own their code and private state. Focused subfolders separate codecs, lifecycle, adapters and composition. The [Transport README](src/modules/transport/README.md)
documents its public boundary, limits and integration handoff. VS Code recommendations and
build/test/debug tasks use Rust tooling.

## Protocol reference and issue reporting

The setup plan follows CR2SE revision
[`8e848eb2e70ef7cd4490a753f9e647fdddcd99fd`](https://github.com/beothorn/CR2SE/tree/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd),
particularly its [module contracts](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/ModuleContracts.md)
and [PoC guide](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/PoC.md).
Review changes upstream before each implementation stage and record any new
baseline. This reference does not claim that the protocol is implemented.

Record protocol, pseudocode, contract and vector defects in
[`CR2SE bugs/`](CR2SE%20bugs/README.md). Reports must apply across implementation
languages; Rust-specific issues belong in this project's implementation work.
See [AGENTS.md](AGENTS.md) for development and validation requirements.
