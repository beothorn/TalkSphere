# Peer policy and admission plan

Status: in progress; steps 1-4 are complete.

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
- [ ] **5.** Admit unconfigured authenticated peers under finite per-identity limits.
- [ ] **6.** Apply per-identity quotas and rate limits before accepting work.
- [ ] **7.** Review CR2SE quota and refusal rules and propose language-independent changes upstream.
- [ ] **8.** Track live peer connections by identity.
- [ ] **9.** Send requests to a peer over the connection that peer opened.
- [ ] **10.** Keep client connections alive with pings and bounded reconnects.
- [ ] **11.** Grant finite bootstrap credits and trust by policy.
- [ ] **12.** Publish wanted Storage terms and issue earned credits by policy.
- [ ] **13.** Adjust trust from verified contributions and failures by policy.
- [ ] **14.** Maintain replica sets at a policy target across independent peers.
- [ ] **15.** Schedule staggered checks, renewals and repairs by policy.
- [ ] **16.** Report replica-set state using the CR2SE redundancy states.
- [ ] **17.** Add local commands to inspect policy decisions and their inputs.
- [ ] **18.** Add a deterministic multi-peer simulation harness using real modules.
- [ ] **19.** Model peer churn, unreliable peers and abusive peers in simulations.
- [ ] **20.** Run parameter sweeps and record their results.
- [ ] **21.** Document policy parameters, defaults and help.
- [ ] **22.** Exercise unconfigured peers, reverse requests and policy decisions between node processes.
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

### 6. Apply per-identity quotas and rate limits before accepting work

Apply quota-policy limits per identity (pending operations, bytes, leases and
request rate) before quotes are accepted, refusing with `resource_limit` and no
charge. Rate-limit failed authentication and identifier probing without
revealing whether a resource exists. Test each limit at its boundary and that
refusals cost nothing.

### 7. Review CR2SE quota and refusal rules and propose language-independent changes upstream

Check whether a requester can tell a temporary quota refusal from a permanent
limit, and whether providers can advertise per-requester limits so requesters
choose providers well. Where a gap is confirmed, edit the CR2SE specification
with a language-independent change and acceptance cases, search existing
issues, file one and record it under [CR2SE bugs](../../CR2SE%20bugs/README.md).
Adapt TalkSphere only after the change is accepted.

### 8. Track live peer connections by identity

Keep a bounded registry of authenticated live connections keyed by identity,
updated on connect, close and GOAWAY. Test concurrent connections from one
identity, replacement after reconnect and that the registry never outlives a
closed connection.

### 9. Send requests to a peer over the connection that peer opened

Let requester operations reach a peer through its live inbound connection when
it has no reachable address, opening a new stream from the server side. Add
process tests where both peers initiate requests on one connection, including
a client that disconnects mid-request.

### 10. Keep client connections alive with pings and bounded reconnects

Add a client profile that connects outward to configured servers, keeps each
connection open with PING at a policy interval, and reconnects with bounded
backoff after loss. Test ping timeout, reconnect and the backoff bound.

### 11. Grant finite bootstrap credits and trust by policy

Replace hand-run grants for new identities with an optional credit policy: a
finite first grant and finite trust per new identity, within a total issuance
budget. Off by default. Test the per-identity and total limits, restart, and
that a second identity for the same peer cannot reuse a grant it never earned.

### 12. Publish wanted Storage terms and issue earned credits by policy

Publish wanted Storage terms from policy and accept clients' offers to fulfil
them over their own connections, issuing the agreed credits once the store
succeeds. Uses the existing library support for wanted Storage. Test issuance
exactly once, budget exhaustion and a restart during issuance.

### 13. Adjust trust from verified contributions and failures by policy

Raise or lower trust from verified observations only: passing and failing
checks, completed contributions and refusals, within policy bounds. CR2SE
allows assigning trust after checking a contribution. Test each adjustment,
the bounds and that unverified claims change nothing.

### 14. Maintain replica sets at a policy target across independent peers

Add a requester-side replica manager for data the server hosts: encrypt to the
server's key, place Storage leases at distinct client identities up to the
policy target, and count a copy only after its complete successful result, as
CR2SE Redundancy requires. Test reaching the target, provider independence and
refusal to count unconfirmed copies.

### 15. Schedule staggered checks, renewals and repairs by policy

Schedule paid checks, renewals and replacement copies so they never fall due
together, calling clients over their live connections. An inconclusive check
never counts as proof of another copy. Test staggering, repair below target and
behavior when no client is connected.

### 16. Report replica-set state using the CR2SE redundancy states

Report not redundant, under-replicated, target satisfied and at risk, with the
target, confirmed count and stale observations shown separately. Test each
state transition.

### 17. Add local commands to inspect policy decisions and their inputs

Add `node-ctl policy show` and `node-ctl policy explain --peer ID` to show
current parameters and the observations behind a decision. Test output and
that no secret material is shown.

### 18. Add a deterministic multi-peer simulation harness using real modules

Add a test-only harness that runs many in-process TalkSphere peers with
in-memory adapters, a shared controlled clock and a simulated network. A run is
reproducible from a seed and produces a machine-readable report. Test that two
runs with one seed match exactly.

### 19. Model peer churn, unreliable peers and abusive peers in simulations

Add models for connection churn, storage loss, slow peers, peers that fail
checks and peers that probe or flood. Test each model in isolation.

### 20. Run parameter sweeps and record their results

Add a documented command that sweeps policy parameters (replica target, check
interval, trust steps, quotas) over seeded runs and writes a report to
`docs/policy/SIMULATIONS.md`. Record the first results and the defaults chosen
from them.

### 21. Document policy parameters, defaults and help

Document every parameter in `docs/policy/PARAMETERS.md` and the policy
contracts' companion files, and add detailed help topics for the new commands
and options with CLI tests.

### 22. Exercise unconfigured peers, reverse requests and policy decisions between node processes

Run a server and several client-profile processes: clients connect outward,
earn credits by wanted Storage, the server keeps replica sets at them, a client
disappears and the server repairs. Assert exact balances and redundancy states.

### 23. Complete the policy acceptance evidence and handoff

Write `docs/policy/ACCEPTANCE.md` and a handoff for Messaging, hosted pages and
feeds, update the roadmap and README, run the full check list and record
coverage.
