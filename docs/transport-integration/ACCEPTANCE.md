# Server integration acceptance inventory

Implementation baseline: [CR2SE d40b6278](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f).
The upstream comparison against main on 2026-11-05 is six commits ahead and changes
only website/workflow/root introductory material; no module contracts, pseudocode,
acceptance cases or vectors changed. Retain the existing behavior and fixture revision.
The specification, Transport inputs/outputs, pseudocode, vectors, TR suite,
ModuleContracts and shared acceptance guide were re-read at that revision.

The earlier [Transport matrix](../../src/modules/transport/ACCEPTANCE.md) has historical
pending notes superseded by its Cryptography and persistent Identity supplements.
Real AUTH and directional frame vectors already pass in composition tests, using
protected identities loaded from disk. They do not yet exercise a server process.

| Case | Existing evidence | Integration work still required |
| --- | --- | --- |
| TR-01 | Real persisted Identity handshake/AUTH/frame vectors | Driver ordering observations and separate-process authentication |
| TR-02 | Scripted every-split/coalesced/short-write framing | Repeat with real protected bytes and representative TCP fragmentation |
| TR-03 | Codec bounds, composed maximum DATA, real maximum tagging | Real driver/socket boundary failures before dispatch |
| TR-04 | Handshake state rejection, real crypto negatives, real Identity contact/denial rejection | Full two-role matrix, weak A/R/S negatives, observed no X25519/HKDF before verified AUTH |
| TR-05 | State gates and coalesced AUTH transitions | Real partial AUTH writes, withheld AUTH, same-ID fresh sessions |
| TR-06 | Real vectors and cryptographic integrity regressions | Real driver mutation/replay/old-session rejection and exit cleanup matrix |
| TR-07 | Exact allocation and controlled exhaustion unit tests | Authenticated driver allocation and exhaustion behavior |
| TR-08 | OPEN codecs, registration and scripted routing | Correctly tagged malformed OPEN and owner isolation |
| TR-09 | Scripted interleaving and half-close | Real protected bidirectional consumer exchange |
| TR-10 | Scripted terminal traffic and cancellation | Real protected cancellation without retries |
| TR-11 | Token/deadline and real primitive PING/PONG checks | Driver/socket probe expiry and duplicate handling |
| TR-12 | Session GOAWAY and scripted drain | Runtime signal handling and authenticated socket drain |
| TR-13 | Exact diagnostic vectors and codec rejection | Correctly tagged invalid controls and captured runtime privacy |
| TR-14 | Scripted every-prefix truncation | Real session EOF/restart and completed-result preservation |
| TR-15 | Driver budgets, scheduler, manual deadlines, real TCP readiness | Runtime wake races, slow consumers, shutdown responsiveness and occupancy evidence |
| AT-C01 | Direct ports, typed grants, real adapters | Real runtime owner checks; serialized local module deployment is unsupported |
| AT-C02 | Bounds and typed failures at individual modules | Startup cleanup, worker failure, bounded contacts and explicit unknown outcomes |
| AT-C03 | Protected identity restart, copied contact snapshots | Process restart with stable keys and fresh sessions; no transient handle reuse |
| AT-C04 | Module diagnostics capture and fail-closed decoders | Executable help/output discipline, signals, private-data exclusion and absent-service refusal |

This inventory records outstanding assertions, not passes. Integration plan steps
15–23 close these rows and step 24 records exact executed test names and outcomes.
Fixed vectors use literal upstream bytes; malformed state fixtures must have valid
independently generated tags unless integrity failure itself is under test.
Unbounded sequence traffic is replaced by controlled test-only state injection.

The application owns runtime lifecycle under src/composition; Transport owns wire
state; Identity owns protected credentials; Cryptography owns primitive execution.
No automatic identity replacement, generic signing interface, fake cr2se.peer
handler, service retry or later module behavior is authorized by this milestone.
Independent implementation interoperability, paid recovery and minimum-useful-node
conformance remain later work. No new protocol defect was confirmed by this review.

## Separate-process authentication

`cargo test --locked --test server_peers` creates two distinct protected identities,
starts the actual executable twice and verifies both reported authenticated IDs and
TCP roles. Both processes are restarted with their roles reversed and retain their
public identities. Readiness and shutdown are observed with bounded pipe reads;
SIGTERM completes normally. This is same-implementation process evidence, not
independent interoperability or a service protocol demonstration.

## Real protected framing

The server real_wire suite uses persisted/reloaded native Identity and real
Cryptography through Connection's actual framing/session boundary. Only socket
segmentation and monotonic time are controlled. An independent direct AEAD oracle
matches the literal upstream tag, then supplies correct tags for state tests.
Every OPEN/DATA split and one-byte handshake/coalesced protected tail run with
one-to-three-byte writes and exact pinned HELLO/AUTH output assertions. Real socket
and separate-process behavior are separately covered by server_peers and runtime tests.
