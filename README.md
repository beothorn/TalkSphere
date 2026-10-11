# TalkSphere

TalkSphere is a Rust server implementation and proof of concept for
[CR2SE](https://github.com/beothorn/CR2SE). It will grow module by module into a
node that exchanges services and storage, followed by messaging, public boards,
pages and the other CR2SE capabilities. See the [implementation plan](plans/PLAN.md)
and the [Transport breakdown](plans/1_Transport_module/PLAN.md).

The intended model is ad-free social features supported by shared device
storage and paid backup space. Compatibility with CR2SE clients is an
implementation goal.  

This implementation is focused on being a server (always running), but will also have client 
features for testing and for advanced users. Better clients with ui that are not using the cli must
be implemented separetely.

**Current status:** project setup and Transport steps 1–2, its public interface
and deterministic dependency test helpers, are implemented and tested. There is no
working server, socket listener, handshake or protocol interoperability yet.
`serve` returns an explicit not-implemented error.

## Build and test

Install a current stable Rust toolchain with Cargo, rustfmt and Clippy. The
project uses Rust edition 2024 and currently has no external dependencies.
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

The last command currently exits with status **1**, explaining that Transport
is not implemented. It opens no sockets. With no arguments, the application
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
  modules/
    transport/             public contracts, dependency ports and unit tests
tests/                     executable integration tests
CR2SE bugs/                language-independent protocol bug reports
AGENTS.md                  instructions for ongoing implementation
plans/
  PLAN.md                  staged implementation checklist
  1_Transport_module/
    PLAN.md                Transport steps and matching detailed instructions
```

Protocol modules own their code and private state. Focused subfolders will be
added as implementations grow. The [Transport README](src/modules/transport/README.md)
documents the first boundary and its next steps. VS Code recommendations and
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
