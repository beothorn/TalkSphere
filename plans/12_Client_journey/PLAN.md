# Full client journey plan

Status: planned for review; no step is implemented yet. It depends on the
[Messaging module plan](../11_Messaging_module/PLAN.md) being complete.

This is roadmap milestone 13 and planning folder 12, following Messaging. Two
operators, Alice and Bob, each run their own node and drive it locally with
`node-ctl`. They earn credits from each other, share Storage, exchange
encrypted messages, and each node saves and retrieves a file.

## Steps

- [ ] **1.** Reconcile the journey with the PoC story and the completed node and Messaging evidence.
- [ ] **2.** Extend node configuration with Messaging, wanted Storage and file limits.
- [ ] **3.** Open, recover and install the Messaging owner in the node.
- [ ] **4.** Publish the node's identity encryption credential on its Board.
- [ ] **5.** Add durable catalogs for sent and received messages.
- [ ] **6.** Add a local command to send an encrypted message to a contact.
- [ ] **7.** Add local commands to list, read and verify received messages.
- [ ] **8.** Add local commands to query status, renew and remove sent messages.
- [ ] **9.** Publish a wanted Storage offering from operator configuration.
- [ ] **10.** Add a local command to fulfil a contact's wanted Storage offering.
- [ ] **11.** Issue credits for fulfilled wanted Storage under requester consent.
- [ ] **12.** Add a durable file manifest to the node catalog.
- [ ] **13.** Add a local command to encrypt, split and store a file at a contact.
- [ ] **14.** Add a local command to retrieve, verify and decrypt a stored file.
- [ ] **15.** Document every new command and option in overview and detailed help.
- [ ] **16.** Run the full client journey between two separately operated nodes.
- [ ] **17.** Exercise failures and restarts at each stage of the journey.
- [ ] **18.** Verify logs and replies exclude secrets, plaintext and file contents.
- [ ] **19.** Publish an operator walkthrough of the journey.
- [ ] **20.** Complete the journey acceptance evidence and handoff.

## Baseline, scope and review

Planned on 2026-11-22 against
[CR2SE bc53df4](https://github.com/beothorn/CR2SE/tree/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16).
Recheck it before step 1.

- [First proof of concept: topology, policy and end-to-end story](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/PoC.md#topology-and-initial-policy).
- [Shared acceptance cases](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/AcceptanceTests.md#end-to-end-acceptance).
- [Storage wanted offerings and later operations](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/storage/README.md#prices-of-later-operations).
- [Messaging example: redundant delivery](https://github.com/beothorn/CR2SE/blob/bc53df4041da9e62d9fb0c31bf9c6ff165e72d16/modules/messaging/README.md#17-example-redundant-delivery).

Local starting points are the [node handoff](../../src/composition/node/HANDOFF.md),
the [Storage handoff](../../src/modules/storage/HANDOFF.md) and the Messaging
handoff that Messaging step 38 publishes.

The roadmap goal, interpreted for review:

- **Get credits:** a finite bootstrap grant, then credits earned by fulfilling
  the other node's wanted Storage offering, as the PoC story requires. No
  unlimited balances or automatic trust.
- **Share storage:** each node provides Storage to the other and buys Storage
  from it, paid in provider-issued credits.
- **Send messages:** encrypted Messaging in both directions, with discovery,
  recovery, verified receipts and early removal for release credits.
- **Save and retrieve a file:** a node encrypts a local file to its own key,
  splits it into objects within the Storage object limit, stores them at the
  contact, restarts, and retrieves and verifies the identical file.

In scope: composition under `src/composition/node/`, new `node-ctl` operations
over the existing control socket, process tests and operator docs on Linux.
Out of scope: the CR2SE Node API (the clients are operators using the local
control channel), Discovery, third-party credit transfer, recovery on a fresh
node from a backup (the full-feature PoC), and the remaining optional families.

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
record under [CR2SE bugs](../../CR2SE%20bugs/README.md). No new protocol defect is
asserted by this plan.

## Details

### 1. Reconcile the journey with the PoC story and the completed node and Messaging evidence

Map each journey goal and the applicable PoC story steps to existing node,
Storage and Messaging evidence, and list what only the composed journey can
prove. Record the file-size bound the chunked design supports. Write the gap
inventory in a new `docs/client-journey/ACCEPTANCE.md`. No code changes.

### 2. Extend node configuration with Messaging, wanted Storage and file limits

Add an optional Messaging offering and capacity limits, an optional wanted
Storage offering with its issuance budget, and a maximum file size and chunk
count to `node.json`. Validation stays effect-free and runs before any
filesystem or network effect. Test missing, oversized and inconsistent values,
and that older configurations still load.

### 3. Open, recover and install the Messaging owner in the node

Open and recover the Messaging owner in `owners::open` with the other durable
owners, install its provider handler only when configured, and publish its
offering after recovery. Test startup with and without Messaging, recovery
before admission, and that the Board never lists Messaging when it is absent.

### 4. Publish the node's identity encryption credential on its Board

Publish the `identityEncryption` field from the node's persistent encryption
key. Teach `fetch` to validate a contact's credential and record whether it is
usable. Test publication, a contact with an invalid or missing credential, and
that other offerings remain usable.

### 5. Add durable catalogs for sent and received messages

Persist sent placements (provider, placement ID, message ID, expiry, last known
state) and received messages (sender, message ID, verified payload, receipt) in
protected atomic files, deduplicated by `(senderId, messageId)`. Test reopen,
corrupt or foreign records and write failure.

### 6. Add a local command to send an encrypted message to a contact

Add `node-ctl message send` to encrypt text or a bounded file to a contact's
verified credential, sign it and store it at one or more chosen providers under
`--max-spend`. Opaque payloads require an explicit option. Test success, a
missing credential, insufficient credits and an unknown outcome.

### 7. Add local commands to list, read and verify received messages

Add `node-ctl message inbox` to discover pending messages at a provider under a
spending bound and `node-ctl message read` to recover, verify and decrypt one.
Metadata is shown before paying to recover. Test pagination, deduplication of
replicas, a tampered payload and an exact retry after a lost reply.

### 8. Add local commands to query status, renew and remove sent messages

Add `node-ctl message status`, `renew` and `remove`, verifying recovery
receipts and accepting release credits only at or above a stated minimum. Test
pending and recovered status, a stale expiry and a removal retried after a lost
reply.

### 9. Publish a wanted Storage offering from operator configuration

Publish the configured wanted Storage offering on the Board with requester
issued credits and a finite issuance budget. Test publication, withdrawal when
the budget is exhausted and refusal of inconsistent terms.

### 10. Add a local command to fulfil a contact's wanted Storage offering

Add `node-ctl storage fulfil` so an operator accepts a contact's wanted Storage
terms and acts as provider for the requested object. The command shows the
offered credits before acceptance. Test acceptance, refusal of unacceptable
terms and a withdrawn offering.

### 11. Issue credits for fulfilled wanted Storage under requester consent

Let the wanted offering's publisher authorize the input and issue the agreed
credits once the work completes, using the existing requester-owned consent
and finite issuance. Trust stays an explicit `node-ctl trust` decision. Test
issuance exactly once, refusal or withdrawal without spending, and a restart
during issuance.

### 12. Add a durable file manifest to the node catalog

Record each stored file's name, size, whole-file hash, encryption format and
ordered chunk leases in the protected catalog. Test reopen, a missing chunk
record, a corrupt manifest and duplicate names.

### 13. Add a local command to encrypt, split and store a file at a contact

Add `node-ctl file put` to read a bounded local file, encrypt it to the node's
own key, split the ciphertext into objects of at most 16384 bytes and store each
as a lease under one total `--max-spend`. A partly stored file is reported with
its stored and missing chunks, never as complete. Test small, boundary and
oversized files, insufficient credits mid-file and an unknown chunk outcome.

### 14. Add a local command to retrieve, verify and decrypt a stored file

Add `node-ctl file get` to retrieve every chunk, verify each lease hash and the
whole-file hash, decrypt and write the result atomically to a new path that it
never overwrites. Test an identical round trip after restart, a tampered chunk,
a missing chunk and an existing output path.

### 15. Document every new command and option in overview and detailed help

Add detailed help topics for each new `node-ctl` operation and option, update
the overview, README examples and CLI tests together.

### 16. Run the full client journey between two separately operated nodes

In `tests/journey_process.rs`, run Alice and Bob as separate `serve
--node-root` processes. Alice grants a finite bootstrap; Bob fulfils Alice's
wanted Storage and earns Alice-issued credits; each buys Storage from the
other; each stores a file, restarts and retrieves it identically; they exchange
encrypted messages both ways, check receipts and remove an unneeded copy for
release credits. Assert every balance change exactly.

### 17. Exercise failures and restarts at each stage of the journey

Kill and restart a provider during a file upload, a message recovery and a
removal, and lose replies after commit. Assert one charge per committed
operation, unknown outcomes never retried silently, and no double release
credits or issuance.

### 18. Verify logs and replies exclude secrets, plaintext and file contents

Run the journey at trace level and assert that logs and control replies contain
no private keys, message plaintext, file bytes or foreign placement IDs.

### 19. Publish an operator walkthrough of the journey

Write `docs/client-journey/WALKTHROUGH.md` with the exact commands two operators
run, from `node-init` to file retrieval and message removal, and the expected
output. Verify every command by running it.

### 20. Complete the journey acceptance evidence and handoff

Complete `docs/client-journey/ACCEPTANCE.md`, update the node handoff, this
plan, the roadmap and the root README, run the full check list and record
coverage and known limits.
