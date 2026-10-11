# Transport

Transport owns CR2SE connections, authentication, frames and multiplexed
streams. It consumes Identity, Cryptography, TCP and timers through explicit
boundaries; it does not own service dispatch, balances or application retries.

**Status:** scaffold only. `transport::start()` always returns the typed local
error `TransportError::NotImplemented`, before any I/O. This is not a protocol
wire error or a completed Transport implementation.

## Layout

- `mod.rs`: public entry point and scaffold behavior test.
- `error.rs`: local error type, diagnostic and unit test.

Introduce focused codec, handshake, stream and adapter files/subfolders during
implementation. Keep session state private and test dependency substitutes
separate from usable runtime capabilities. The placeholder `start()` boundary
will evolve to accept explicit dependencies and configuration.

## Protocol references

Initial baseline:
[`8e848eb2e70ef7cd4490a753f9e647fdddcd99fd`](https://github.com/beothorn/CR2SE/tree/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd).

- [Wire specification](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/README.md)
- [Inputs](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/inputs/README.md)
  and [outputs](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/outputs/README.md)
- [Pseudocode](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/pseudocode/README.md),
  [wire vectors](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/pseudocode/TestVectors.md)
  and [acceptance cases TR-01–TR-15](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/acceptanceTests/README.md)

See [PLAN.md](../../../PLAN.md) step 2 for implementation and steps 3–5 for
real Identity/Cryptography integration. No acceptance case is claimed as passed
by this scaffold.

## Build, run and test

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::transport
cargo run --locked -- help serve
cargo run --locked -- serve
```

The final command currently reports the missing Transport implementation on
stderr and exits with status 1. It starts no listener. Run all tests with
`cargo test --locked`; build and run details are also in the
[project README](../../../README.md).
