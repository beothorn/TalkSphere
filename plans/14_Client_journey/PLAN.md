# Full client journey plan

Status: planned for review; no step is implemented yet. It depends on
[peer policy](../12_Peer_policy/PLAN.md), [Messaging](../11_Messaging_module/PLAN.md),
the [hosted pages and feeds specification](../13_Hosted_publications/PLAN.md) and
the Static Pages and Public Feeds modules planned from it.

This is roadmap milestone 17 and planning folder 14. It proves, entirely over
CR2SE and before any Android work starts, that the deployment the messenger
needs works: clients that only connect outward use TalkSphere servers to
exchange messages, publish profile pages and post to feeds, and pay with their
own resources.

## Steps

- [ ] **1.** Reconcile the journey with the PoC story, the Redundancy rules and the completed evidence.
- [ ] **2.** Compose Messaging, hosted pages and hosted feeds providers in server nodes.
- [ ] **3.** Publish each node's identity encryption credential on its Board.
- [ ] **4.** Let client nodes earn server credits by providing Storage over their own connections.
- [ ] **5.** Back up server-hosted data as replica sets at connected clients.
- [ ] **6.** Add client catalogs for messages, followed feeds and published pages.
- [ ] **7.** Add local commands to send, list, read and manage messages.
- [ ] **8.** Add local commands to publish, update and read profile pages.
- [ ] **9.** Add local commands to post to and read feeds.
- [ ] **10.** Document every new command and option in overview and detailed help.
- [ ] **11.** Run the journey across two servers and several clients.
- [ ] **12.** Exercise client disconnection, server restart and replica repair.
- [ ] **13.** Exercise unknown outcomes, quota refusals and insufficient credits.
- [ ] **14.** Verify logs and replies exclude secrets, plaintext and content.
- [ ] **15.** Simulate the journey at scale with the policy harness.
- [ ] **16.** Publish an operator walkthrough of the journey.
- [ ] **17.** Complete the journey acceptance evidence and handoff.

## Baseline, scope and review

Planned on 2026-11-22 against
[CR2SE bc53df4](https://github.com/beothorn/CR2SE/tree/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16);
recheck before step 1, because the hosted pages and feeds change will move
upstream.

- [PoC topology, policy and end-to-end story](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/PoC.md#topology-and-initial-policy).
- [Redundancy](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/Redundancy.md).
- [Messaging example: redundant delivery](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/messaging/README.md#17-example-redundant-delivery).

Topology:

- **Servers:** two TalkSphere nodes, S1 and S2, that accept connections and
  provide Messaging, hosted pages and hosted feeds. Two servers show that a
  publication or message placed at several providers survives one of them.
- **Clients:** several TalkSphere nodes in the client profile from peer policy.
  They stand in for phones: they never listen, they connect outward to both
  servers, keep the connections open and go offline at times.
- **Credits:** clients earn server-issued credits by providing Storage for data
  the servers host, and spend them on Messaging, pages and feeds at that
  server. A finite bootstrap grant is a policy option, off by default.
- **Redundancy:** each server keeps its hosted data as replica sets at
  connected clients, calling them over their own connections. Tests use a
  small target; the planned default of about ten is evaluated in simulation.

The clients are driven through `node-ctl`. Every command maps directly to CR2SE
requester operations; application conventions (how a profile looks, who
follows whom, chat history) stay in the client catalogs and never in the
server. The Android app will reimplement this client side in Kotlin.

Out of scope: interoperability with other implementations, Discovery (clients
exchange contact QR codes or IDs), the Node API, public file sharing and
computation.

## Execution rules

Follow [AGENTS.md](../../AGENTS.md) for each step: unit tests with failure and
boundary cases, process-level tests, entry tracing, companion contract
Markdown, current README and help, and production coverage above 80% measured
with [the coverage procedure](../../docs/coverage.md). Keep composition under
`src/composition/`, CLI behavior under `src/cli/` and protocol fixes in their
owning modules. Commit each verified step separately with a descriptive title
and body. Check a step only after its tests and docs pass.

Search open and closed upstream issues before reporting a confirmed protocol
defect, file new language-independent findings on GitHub and keep the local
record under [CR2SE bugs](../../CR2SE%20bugs/README.md).

## Details

### 1. Reconcile the journey with the PoC story, the Redundancy rules and the completed evidence

Map each journey goal to existing evidence from the minimum node, peer policy,
Messaging, pages and feeds, and list what only the composed journey proves.
Write the gap inventory in `docs/client-journey/ACCEPTANCE.md`. No code changes.

### 2. Compose Messaging, hosted pages and hosted feeds providers in server nodes

Open and recover the new durable owners in `owners::open`, install each
provider only when configured, publish offerings after recovery, and attach
them to the peer policy contracts. Test startup with each family present or
absent and that the Board lists only installed families.

### 3. Publish each node's identity encryption credential on its Board

Publish the `identityEncryption` field from the node's persistent encryption
key, and let `fetch` validate a contact's credential. Test publication and a
contact with an invalid or missing credential.

### 4. Let client nodes earn server credits by providing Storage over their own connections

Configure the server's wanted Storage through peer policy and let a client
fulfil it over its outbound connection, receiving server-issued credits once.
Test earning, budget exhaustion and a client disconnecting mid-store.

### 5. Back up server-hosted data as replica sets at connected clients

Use the peer policy replica manager to keep the servers' hosted messages,
pages and feeds, encrypted to the server's key, at distinct clients, checking
and repairing them over the clients' connections. Test the target, repair when
a client leaves, and recovery of hosted data from client replicas after the
server loses its local copy.

### 6. Add client catalogs for messages, followed feeds and published pages

Persist sent and received messages deduplicated by `(senderId, messageId)`,
followed feeds with their cursors, and the client's published pages and posts
with every host's placement. Atomic, owner-only files. Test reopen, corrupt or
foreign records and write failure.

### 7. Add local commands to send, list, read and manage messages

Add `node-ctl message send`, `inbox`, `read`, `status`, `renew` and `remove`.
Send encrypts to the contact's verified credential and stores the identical
signed message at the chosen servers; read verifies and decrypts; status
verifies receipts. Test each under spending bounds, including replicas,
tampered payloads and an exact retry after a lost reply.

### 8. Add local commands to publish, update and read profile pages

Add `node-ctl page publish`, `update` and `get` following the accepted hosted
pages specification: a signed page snapshot placed at several servers, public
or restricted to chosen readers. Reading picks the latest verified version
across servers. Test publication, update, restricted access and a stale host.

### 9. Add local commands to post to and read feeds

Add `node-ctl feed post` and `feed read` following the accepted hosted feeds
specification, with cursors and a merged local view of followed feeds. Test
posting at several servers, reading with cursors, merging and a missing host.

### 10. Document every new command and option in overview and detailed help

Add detailed help topics for each new `node-ctl` operation and option, update
the overview, README examples and CLI tests together.

### 11. Run the journey across two servers and several clients

In `tests/journey_process.rs`, run S1, S2 and the clients as separate
processes. Clients earn credits by providing Storage, publish profile pages and
feed posts at both servers, read each other's pages and feeds, and exchange
encrypted messages while the recipient is offline, then check receipts and
remove an unneeded copy for release credits. Each server keeps its hosted data
at clients. Assert every balance change and redundancy state exactly.

### 12. Exercise client disconnection, server restart and replica repair

Disconnect clients during uploads and recoveries, restart a server, and stop
one server entirely. Assert that messages and publications remain available
from the other server, that hosted data is repaired from client replicas, and
that no committed operation is charged twice.

### 13. Exercise unknown outcomes, quota refusals and insufficient credits

Lose replies after commit, exceed per-identity quotas and run out of credits.
Assert unknown outcomes are never retried silently, refusals cost nothing and
release credits or issuance never apply twice.

### 14. Verify logs and replies exclude secrets, plaintext and content

Run the journey at trace level and assert that logs and control replies contain
no private keys, message plaintext, restricted page or feed content, or foreign
placement IDs.

### 15. Simulate the journey at scale with the policy harness

Run the journey's flows in the peer policy simulation harness with many
clients, churn and abusive peers, sweeping the replica target and check
cadence, and record the results in `docs/policy/SIMULATIONS.md`.

### 16. Publish an operator walkthrough of the journey

Write `docs/client-journey/WALKTHROUGH.md` with the exact commands to start two
servers and several clients and run every journey step, with expected output.
Verify each command by running it. This is also the reference for the Kotlin
client.

### 17. Complete the journey acceptance evidence and handoff

Complete `docs/client-journey/ACCEPTANCE.md`, write a handoff for the Android
client listing every CR2SE operation and policy it relies on, update the node
handoff, this plan, the roadmap and the root README, run the full check list and
record coverage and known limits.
