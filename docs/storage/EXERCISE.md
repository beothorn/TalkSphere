# Native Storage exercise

From the repository root on Linux with loopback networking:

```sh
cargo build --locked
cargo run --locked -- help storage
cargo run --locked --example storage -- --help
cargo run --locked --example storage -- help run
cargo run --locked --example storage -- run
cargo test --locked --example storage
```

Run creates private temporary identities, independent native Board, Services and
Ledger journals, and a protected Storage owner. The nodes authenticate pinned
public identities over loopback. The requester obtains the provider's Board and
definition directly and consents to each exact quote with finite demonstration
credits. A 5000-byte upload and the full retrieval use detached bodies.

The exercise creates one 60-second lease, closes the provider, reopens its
original native stores under the same identity and retrieves the exact bytes.
It then challenges one byte using a private expected value, renews the lease
for five more seconds and removes it. Confirmed balances on both owners equal
the five exact charges. After removal, a new retrieval fails and leaves those
balances unchanged. One lease is one copy at one provider.

The explicit example uses protected temporary directories and removes them
after shutdown. The provider reopens within the same Linux boot using kernel
uptime and boot identity. No operator state is required. The integration suite
also runs the provider in a separate process and reauthenticates after restarting
it against the same journals.

Help exits 0 without opening stores or sockets. Invalid syntax exits 2 and a
runtime failure exits 1. Use `RUST_LOG=talksphere::modules::storage=trace` for
Storage traces on stderr. The completion line goes to stdout. The ordinary
`serve` profile remains independent of this explicit exercise.

The tests exercise a provider-opened wanted store with requester-owned input
and finite issuance, then spend part of those earned credits at their issuer.
They also store independently encrypted replicas at two providers, save the
requester key backup and catalog separately, restore the original identity and
recover and decrypt from one provider while the other is offline. Two leases
at a single provider count as one provider.

After one provider is lost the replica test also rebuilds the copy at a new
independent identity. Interruption tests cover an interrupted upload, a lost
result, a lost paid-check reply, a confirmed missing byte and a foreign
requester, and a settlement test pays through an independent third-party issuer.
This is a native loopback profile; default `serve` does not install the paid
Storage handler.
