# Peer policy and admission plan

Status: in progress; steps 1-22 are complete.

This is roadmap milestone 12 and planning folder 12, following the minimum
useful node. CR2SE deliberately leaves trust, credit bootstrap, quotas,
provider selection and redundancy to each identity's local policy. The minimum
node hard-codes or hand-operates all of them: the operator runs `node-ctl grant`
and `trust` for every peer and reaches peers only through configured contacts.
TalkSphere must instead serve many unknown, intermittently connected clients,
such as phones running a Kotlin implementation based on TalkSphere.

This milestone isolates every such decision behind explicit policy contracts
with parameters in one validated file, so they can be tuned from real usage and
large simulations without touching protocol code.

## Steps

- [x] **1.** Inventory every decision CR2SE leaves to local policy and where TalkSphere fixes it today.
- [x] **2.** Define policy contracts that decide only from recorded observations.
- [x] **3.** Load policy parameters from a validated policy file.
- [x] **4.** Record per-peer observations durably and within fixed bounds.
- [x] **5.** Admit unconfigured authenticated peers under finite per-identity limits.
- [x] **6.** Apply per-identity quotas and rate limits before accepting work.
- [x] **7.** Review CR2SE quota and refusal rules and propose language-independent changes upstream.
- [x] **8.** Track live peer connections by identity.
- [x] **9.** Send requests to a peer over the connection that peer opened.
- [x] **10.** Keep client connections alive with pings and bounded reconnects.
- [x] **11.** Grant finite bootstrap credits and trust by policy.
- [x] **12.** Publish wanted Storage terms and issue earned credits by policy.
- [x] **13.** Adjust trust from verified contributions and failures by policy.
- [x] **14.** Maintain replica sets at a policy target across independent peers.
- [x] **15.** Schedule staggered checks, renewals and repairs by policy.
- [x] **16.** Report replica-set state using the CR2SE redundancy states.
- [x] **17.** Add local commands to inspect policy decisions and their inputs.
- [x] **18.** Add a deterministic multi-peer simulation harness using real modules.
- [x] **19.** Model peer churn, unreliable peers and abusive peers in simulations.
- [x] **20.** Run parameter sweeps and record their results.
- [x] **21.** Document policy parameters, defaults and help.
- [x] **22.** Exercise unconfigured peers, reverse requests and policy decisions between node processes.
- [ ] **23.** Complete the policy acceptance evidence and handoff.

## Baseline, scope and review

Planned on 2026-11-22 against
[CR2SE bc53df4](https://github.com/beothorn/CR2SE/tree/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16).
Recheck it before step 1.

- [Trust and bootstrap](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/ledger/README.md#trust-and-bootstrap):
  unknown identities start at zero trust, and a provider must not supply a paid
  service while its trust in that requester is zero.
- [Redundancy](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/Redundancy.md):
  replica sets, independence, repair and the reported redundancy states.
- [Peer Protocol failure codes](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/peer-protocol/README.md),
  including `resource_limit` for capacity refusals.
- [Transport streams and PING](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/transport/README.md):
  either peer may open streams on one connection; ping timeouts are local policy.
- [PoC topology and initial policy](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/PoC.md#topology-and-initial-policy).

Deployment model this plan serves: a reachable TalkSphere server and many
clients that only connect outward and keep the connection open. Over that
connection the server can call a client, for example to store, check or
retrieve a replica. Clients earn server-issued credits by providing Storage
for data the server hosts. The server keeps that data as replica sets at
several clients (initially about ten), to make up for each phone being an
unreliable provider. The target count, selection, check cadence and repair
rules are parameters, not constants, and are expected to change after real
measurements.

Design rules:

- Policies are deterministic functions of recorded observations, configuration
  and the clock. They never read peer-supplied descriptions as instructions, and
  they never change protocol bounds or exact values.
- Every policy decision stays within CR2SE rules. Automatic trust is only ever
  a finite, operator-configured policy; the default keeps it off, as in the
  minimum node.
- Protocol modules expose the decision points; the policy implementations live
  in composition (`src/composition/policy/`), so another deployment can replace
  them without forking protocol code.
- The simulation harness runs real module code with in-memory adapters and a
  controlled clock. It is test and research tooling, never production
  networking.

In scope: Storage, which is the only installed family today. Messaging,
hosted pages and feeds attach to the same contracts in their own milestones.
Out of scope: interoperability with other implementations (TalkSphere only
needs to work with itself for now), the Node API and Discovery.

The user authorized changes to CR2SE itself where policy needs a protocol
clarification. Such changes stay language-independent: no Rust-specific types,
crates or tooling in CR2SE.

## Execution rules

Follow [AGENTS.md](../../AGENTS.md) for each step: unit tests with failure and
boundary cases, process-level tests, entry tracing, companion contract
Markdown, current README and help, and production coverage above 80% measured
with [the coverage procedure](../../docs/coverage.md). Commit each verified step
separately with a descriptive title and body. Check a step only after its tests
and docs pass.

Search open and closed upstream issues before reporting a confirmed protocol
defect, file new language-independent findings on GitHub and keep the local
record under [CR2SE bugs](../../CR2SE%20bugs/README.md).

## Details

### 1. Inventory every decision CR2SE leaves to local policy and where TalkSphere fixes it today

List each local-policy decision in CR2SE: trust assignment and adjustment,
bootstrap grants, issuance budgets, wanted-offering terms, admission, quotas,
rate limits, provider selection, replica count, independence, check and renewal
cadence, repair, release-credit amounts, ping and reconnect timing. For each,
record where TalkSphere decides it today (constant, `node.json`, `node-ctl` or
absent). Write `docs/policy/INVENTORY.md`. No code changes.

Completed: [Inventory](../../docs/policy/INVENTORY.md) with seventeen decisions mapped to six contracts.

### 2. Define policy contracts that decide only from recorded observations

Define small contracts in `src/composition/policy/`: admission, quota, trust,
credit, redundancy and connection. Each takes an explicit observation snapshot
and returns a typed decision with a reason. Provide conservative default
implementations that reproduce today's behavior. Test that the defaults change
nothing observable in the existing node tests.

Completed: [Contracts](../../src/composition/policy/contracts.md) and [defaults](../../src/composition/policy/defaults.md) with unit tests; they are wired into the node from step 5.

### 3. Load policy parameters from a validated policy file

Add a versioned `policy.json` beside `node.json`, validated before any effect,
with finite bounds on every parameter and documented defaults. Unknown members
and inconsistent values are refused. Test missing, oversized and inconsistent
parameters and a policy file absent entirely.

Completed: [Parameters](../../src/composition/policy/params.md) and [configured policy](../../src/composition/policy/configured.md); the node refuses an invalid `policy.json` at startup and `node-show` reports its state. `Defaults` was replaced by `Params::default()`.

### 4. Record per-peer observations durably and within fixed bounds

Persist bounded per-identity observations: first seen, connection history,
contributions, check outcomes, refusals and failures. Old entries age out by a
fixed rule; peer-supplied values are bounded and never logged as trusted facts.
Test reopen, the bounds, ageing and corrupt records.

Completed: [Observations](../../src/composition/policy/observations.md) and the [durable store](../../src/composition/node/observation_store.md). Grants are written immediately; other events flush periodically. The runtime starts recording in step 5.

### 5. Admit unconfigured authenticated peers under finite per-identity limits

Confirm by test whether the node already serves an authenticated identity that
is not a configured contact, and make admission an explicit admission-policy
decision. Board and definition reads stay free but rate-limited, as CR2SE
allows. Add process tests for an unconfigured peer reading the Board and being
refused paid work at zero trust.

Completed: admission is decided by the policy contract when a peer-opened connection authenticates, using the live counts and observations; process tests show an unconfigured peer reading metadata and being refused paid work at zero trust, and a connection limit closing the second peer. Free reads are rate-limited in step 6.

### 6. Apply per-identity quotas and rate limits before accepting work

Apply quota-policy limits per identity (pending operations, bytes, leases and
request rate) before quotes are accepted, refusing with `resource_limit` and no
charge. Rate-limit failed authentication and identifier probing without
revealing whether a resource exists. Test each limit at its boundary and that
refusals cost nothing.

Completed: [Rates](../../src/composition/policy/rates.md) and the [runtime quota rules](../../src/composition/node/runtime.md). Rate and failure limits close the offending connection (the stream layer has no per-identity reply hook); the pending limit tightens `capacity.perOwner` and is uniform across peers. Step 7 reviews whether CR2SE should define a clearer refusal for rate limits.

### 7. Review CR2SE quota and refusal rules and propose language-independent changes upstream

Check whether a requester can tell a temporary quota refusal from a permanent
limit, and whether providers can advertise per-requester limits so requesters
choose providers well. Where a gap is confirmed, edit the CR2SE specification
with a language-independent change and acceptance cases, search existing
issues, file one and record it under [CR2SE bugs](../../CR2SE%20bugs/README.md).
Adapt TalkSphere only after the change is accepted.

Completed: [Review](../../docs/policy/QUOTA_REVIEW.md). Found two confirmed gaps (no retry guidance on `resource_limit`, no specified rate-limit refusal) and a third deferred until simulations; drafted the CR2SE change (optional `retryAfterSeconds`, case PP-09A) and committed it in the CR2SE checkout, unpushed. No upstream issue was filed because it is a design proposal, not a confirmed defect; TalkSphere is unchanged until it is accepted.

### 8. Track live peer connections by identity

Keep a bounded registry of authenticated live connections keyed by identity,
updated on connect, close and GOAWAY. Test concurrent connections from one
identity, replacement after reconnect and that the registry never outlives a
closed connection.

Completed: [Peers](../../src/composition/node/peers.md), wired through `Link` and reported by `node-ctl status`; a process test follows a connection until it closes. GOAWAY surfaces as a close observation. Done before step 5 because admission needs the counts.

### 9. Send requests to a peer over the connection that peer opened

Let requester operations reach a peer through its live inbound connection when
it has no reachable address, opening a new stream from the server side. Add
process tests where both peers initiate requests on one connection, including
a client that disconnects mid-request.

Completed: sessions reuse the live connection whichever side opened it and contacts may be IDs alone; a process test has a server buy storage from a client over the connection the client opened, then fail at once after the client disconnects. A disconnect during an in-flight exchange is not repeated here: it uses the same unknown-outcome machinery proven by the minimum node tests.

### 10. Keep client connections alive with pings and bounded reconnects

Add a client profile that connects outward to configured servers, keeps each
connection open with PING at a policy interval, and reconnects with bounded
backoff after loss. Test ping timeout, reconnect and the backoff bound.

Completed: [Upkeep](../../src/composition/node/upkeep.md) and the [client profile](../../src/composition/node/runtime.md): `serve --node-root --contact` keeps contacts connected with policy-bounded backoff and pings every connection. Process tests restart a crashed server and freeze one with SIGSTOP; the client detects the silent connection by ping timeout and reconnects.

### 11. Grant finite bootstrap credits and trust by policy

Replace hand-run grants for new identities with an optional credit policy: a
finite first grant and finite trust per new identity, within a total issuance
budget. Off by default. Test the per-identity and total limits, restart, and
that a second identity for the same peer cannot reuse a grant it never earned.

Completed: `Owners::bootstrap` (record first, then Ledger grant and trust raise) applied when a peer-opened connection authenticates; a process test shows two peers granted, the third refused when the budget is used up, spending, and no repeat after a provider crash and restart. A second identity is a new identity and gets its own grant while the budget lasts: identity-cost limits are a policy choice documented in the parameters, not something CR2SE can prevent.

### 12. Publish wanted Storage terms and issue earned credits by policy

Publish wanted Storage terms from policy and accept clients' offers to fulfil
them over their own connections, issuing the agreed credits once the store
succeeds. Uses the existing library support for wanted Storage. Test issuance
exactly once, budget exhaustion and a restart during issuance.

Completed: [Wanted](../../src/composition/node/wanted.md): the Board publishes `storage-wanted` from `credit.wanted`, `Work` is the publisher policy and a completion thread validates and catalogs each contribution. A process test shows a client earning exactly the published credits, the server keeping the lease, a second contribution refused by the finite issuance budget without charge, and state surviving a restart. A crash in the middle of issuance relies on the library's coordinator recovery tests; it is not repeated at process level. The client journey plan's `storage fulfil` is the `fulfil` command added here.

### 13. Adjust trust from verified contributions and failures by policy

Raise or lower trust from verified observations only: passing and failing
checks, completed contributions and refusals, within policy bounds. CR2SE
allows assigning trust after checking a contribution. Test each adjustment,
the bounds and that unverified claims change nothing.

Completed: `Owners::observe` records the event and applies the policy's new trust. Verified wanted contributions, conclusive paid checks and peer-attributed failures feed it; an inconclusive check does not. A process test shows trust rising by contribution and passed-check steps to the ceiling, never above it, and falling after a failed check, starting from the operator's explicit value.

### 14. Maintain replica sets at a policy target across independent peers

Add a requester-side replica manager for data the server hosts: encrypt to the
server's key, place Storage leases at distinct client identities up to the
policy target, and count a copy only after its complete successful result, as
CR2SE Redundancy requires. Test reaching the target, provider independence and
refusal to count unconfirmed copies.

Completed: [Replicas](../../src/composition/node/replicas/mod.md). Items are sealed with a node data key (XChaCha20-Poly1305; the key must be backed up with the identity) rather than the generic identity envelope, because the running node does not expose its identity's decryption authority; the sealer is a separate file so the envelope can replace it. Placement goes through the wanted offering because a provider must offer in CR2SE: the node does not push stores to clients, and phone-issued credits are not needed to place. Checks, renewals and retrieval, which are charged in the holder's credits, are step 15. A process test shows distinct clients receiving an item until the target, a repeat client and a third client refused without payment, sealed bytes only at the clients, and state kept across a restart.

### 15. Schedule staggered checks, renewals and repairs by policy

Schedule paid checks, renewals and replacement copies so they never fall due
together, calling clients over their live connections. An inconclusive check
never counts as proof of another copy. Test staggering, repair below target and
behavior when no client is connected.

Completed: [Maintenance](../../src/composition/node/replicas/maintain.md): a pure planner and a one-second loop perform due checks and renewals over the holders' live connections, paid in their credits within `redundancy.maxSpend`, with expiry and conclusive failures dropping the replica. New redundancy parameters: `renewBeforeSeconds`, `maxSpend`, `retrySeconds`; `checkSeconds` may now be as low as one second so it can be exercised. The server cannot push a replacement to a client: repair means the dropped item is offered to the next client that fulfils the wanted offering. Process tests cover check and renewal, an unreachable holder expiring and the item being replicated again, and a failed check removing the replica and lowering trust.

### 16. Report replica-set state using the CR2SE redundancy states

Report not redundant, under-replicated, target satisfied and at risk, with the
target, confirmed count and stale observations shown separately. Test each
state transition.

Completed: [Report](../../src/composition/node/replicas/report.md) shown by `node-ctl replicas`: the state comes from unexpired confirmed copies only, the target and confirmed count are separate, a target of one is met without being redundant, and risk and stale observations are listed.

### 17. Add local commands to inspect policy decisions and their inputs

Add `node-ctl policy-show` and `node-ctl policy-explain --peer ID` to show
current parameters and the observations behind a decision. Test output and
that no secret material is shown.

Completed: `policy-show` and `policy-explain` (hyphenated like the other actions); a process test checks parameters, history, decisions and a malformed identity refusal. Per-minute rate counters live in the node loop and are not shown.

### 18. Add a deterministic multi-peer simulation harness using real modules

Add a test-only harness that runs many in-process TalkSphere peers with
in-memory adapters, a shared controlled clock and a simulated network. A run is
reproducible from a seed and produces a machine-readable report. Test that two
runs with one seed match exactly.

Completed: [Simulation](../../src/composition/node/sim/mod.md). It runs the real policy, observation table, replica book (volatile backing), maintenance planner and redundancy report against a modelled network, Storage and prices; the wire protocol is not exercised there. Runs are reproducible from a seed and report machine-readable measurements including retrievability.

### 19. Model peer churn, unreliable peers and abusive peers in simulations

Add models for connection churn, storage loss, slow peers, peers that fail
checks and peers that probe or flood. Test each model in isolation.

Completed: churn, storage loss, slow peers and dishonest peers in the [world](../../src/composition/node/sim/world.md), flooding in the [abuse experiment](../../src/composition/node/sim/abuse.md), each with an isolated test. Dishonest peers are not excluded from placements by any policy yet; the reported credits they receive is the evidence step 20 weighs before one is added.

### 20. Run parameter sweeps and record their results

Add a documented command that sweeps policy parameters (replica target, check
interval, trust steps, quotas) over seeded runs and writes a report to
`docs/policy/SIMULATIONS.md`. Record the first results and the defaults chosen
from them.

Completed: the `policy_sim` example (`run`, `abuse`, `sweep`) and [SIMULATIONS.md](../../docs/policy/SIMULATIONS.md) with the first results: a target of about ten gives about 99 % retrievability on the default flaky fleet, daily checks are as good as hourly ones, and a request limit of 120 a minute stops flooding without refusing honest peers in the model. The built-in defaults stay conservative; the chosen values are the [phone-fleet profile](../../docs/policy/phone-fleet.policy.json). The sweep also showed that dishonest clients are paid in proportion to their share because no placement rule excludes them yet; that follow-up is recorded in the results.

### 21. Document policy parameters, defaults and help

Document every parameter in `docs/policy/PARAMETERS.md` and the policy
contracts' companion files, and add detailed help topics for the new commands
and options with CLI tests.

Completed: [PARAMETERS.md](../../docs/policy/PARAMETERS.md) lists every parameter, the contract and module guides were updated as each step landed, `talksphere help policy` was added, the node-ctl, node-show and serve option help covers every new action, a CLI test checks discoverability, and the root README has a peer policy section.

### 22. Exercise unconfigured peers, reverse requests and policy decisions between node processes

Run a server and several client-profile processes: clients connect outward,
earn credits by wanted Storage, the server keeps replica sets at them, a client
disappears and the server repairs. Assert exact balances and redundancy states.

Completed: `a_server_keeps_a_replica_set_at_outbound_clients_and_repairs_it` in `tests/policy_process.rs` runs a server and three client-profile processes: the clients connect outward and offer over their own connections, two earn exactly 5 credits each and the set reaches `target satisfied`, the third is refused and earns nothing, maintenance is paid in the holders' credits, a holder crashes, its copy expires and the state falls to `not redundant`, and the next client's offer repairs the set. The test exposed a real defect fixed in the same change: a copy whose retention had ended still counted against the target and still blocked its holder until maintenance dropped it, so the repair was refused for up to a second; the book now takes the clock and counts only unexpired copies. The simulation figures were regenerated after that fix.

### 23. Complete the policy acceptance evidence and handoff

Write `docs/policy/ACCEPTANCE.md` and a handoff for Messaging, hosted pages and
feeds, update the roadmap and README, run the full check list and record
coverage.
