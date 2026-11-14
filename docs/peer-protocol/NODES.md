# Real local Peer Protocol node evidence

Run from the repository root on Linux with loopback sockets permitted:

```sh
cargo build --locked
cargo test --locked --lib modules::peer_protocol::engine::endpoint::tests::network -- --nocapture
cargo test --locked --test peer_protocol_streams
```

The first command builds the application. The first test command exercises actual
Identity, Cryptography, authenticated TCP Transport, Board, Services and Ledger
through the production bridge and endpoint. The second test command separately
checks real TCP malformed-stream isolation. These are same-implementation tests;
they do not demonstrate interoperability with an independent implementation.

## Explicit profiles and ownership

The test-owned custom Echo handler has an independently installed contract,
fixed seven-credit ordinary work, detached UTF-8 input/output, and an existing
five-byte resource for public offset challenges. Paid evidence costs three
credits; included evidence has a one-use local scope. These are explicit custom
fixtures, not Storage or Messaging providers. Local policy alone supplies wanted
input and separate quote spending consent. No test bypasses real authentication,
cryptographic proof, Ledger reservation or Services commitment.

Every node has a randomly located owner-only protected Identity store and real
scoped credentials. The process test starts its provider and issuer as independent
child processes of the requester test process, each running the real node poll
loop. Readiness reports the actual ephemeral loopback address through bounded
local stdout. Expected public identities are pinned before connecting. No fixed
port, startup sleep, private-key export or authentication substitute is used.

The provider/requester service journals and custom resources are explicitly
volatile in these network exercises. The independent issuer's Ledger uses the
actual protected Linux FileStore and is reopened by a fresh process after exit.
These tests establish issuer-record restart durability, not durable custom Echo
resources or full service crash recovery. Services and Ledger's separate durable
recovery tests cover their own owner contracts and uncertainty gates.

## Executable scenarios

| Test | Evidence |
| --- | --- |
| `three_node_processes_complete_work_and_durable_issuer_replay` | Three node processes complete direct metadata retrieval, exact-schema detached multibyte work, paid evidence and real owner-signed transfer negotiation; a fresh issuer process retains the91-credit source balance and original gross9/fee2/net7 result. The same signed request with maximum fee zero and incompatible fresh consent still returns the historical result without a new fee offer or charge. |
| `authenticated_wanted_bodies_and_included_evidence_complete_between_nodes` | Two actual TCP nodes complete provider-opened wanted work with publisher-selected detached input/output and publisher requester identity, then independently retrieve provided metadata and complete included evidence. This caught and now regresses missing requester-body routing in the incoming wanted handler. |
| `real_nodes_settle_third_party_work_and_preserve_interrupted_control_uncertainty` | Three actual TCP nodes commit seven-credit work under a preagreed gross9/fee2 transfer; requester and provider use separate issuer connections and normally completed opaque observations to settle their own books to91 and7 once. Local connection-instance keys intentionally differ across nodes. Closing a constructed control before remote processing yields Unknown; an unrelated issuer connection continues successfully. |
| `authenticated_metadata_and_malformed_stream_isolation_over_real_tcp` | Real protected identities and actual crypto complete two metadata streams while a malformed sibling receives only PROTOCOL_ERROR and the connection stays usable. |

The two-node and complete third-party scenarios execute in one process with
independent module owners; the named three-process test supplies process-boundary
and persistent issuer restart evidence. Test-owned scope observations record only
after the real factory opens; they neither create authentication nor authorize
financial effects. Public signatures are produced through a protected Identity
capability before Transport acquires that credential store's exclusive lock.

The internal `node_process` test is inert unless its own parent supplies a private
fixture configuration. It is not an operator command or remote administration
API. Child processes are bounded, stopped and reaped; temporary fixtures are
removed. Network failures retain typed errors and original module ownership.

Committed reply loss, explicit cached recovery/removal, accepted aborts, late
worker completion and included-delivery ambiguity also have focused handler/module
regressions. In particular, the open upstream
[paid-check timeout settlement issue](https://github.com/beothorn/CR2SE/issues/10)
prevents a global no-charge-on-lost-response claim. No new acknowledgement,
automatic retry, refund or fabricated rollback is introduced by these exercises.
