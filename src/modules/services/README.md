# Services

Services implements reusable typed validation, explicit handler registration and
durable local coordination over real Board and Ledger public contracts. Board
uses the schema validator and registered capabilities; trusted application
composition supplies authority and installed handlers; Peer Protocol will supply
authenticated stream events and wire encoding. No application protocol is yet
registered in production serve.

From the repository root:

```sh
cargo build --locked
cargo test --locked modules::services
cargo run --locked --example services
cargo test --locked --example services
cargo run --locked -- help services
```

The example explicitly registers a custom text handler and settles seven credits
from a finite hundred-credit grant (`price=7, remaining=93`). All example stores
and handler effects are volatile: each run starts fresh, and restart recovery is
not offered by the example. No standard-family runtime or network provider is
installed. `--help`/`-h` explains example usage and exit statuses.

For durable integration, composition must initialize absent Services, Ledger and
family stores explicitly, retain exact installed recovery code, and issue separate
scoped grants to each owner. Linux FileStore uses protected owner-only directories,
exclusive locking and synchronized atomic complete-image replacement. On Unknown,
stop affected owners, reopen all authoritative stores with fresh grants, then
resume original-key status and decisions. Never initialize missing history as a
repair, infer rollback from timeout or repeat execution to recover a lost reply.

The [implementation plan](../../../plans/7_Services_module/PLAN.md),
[requirements](REQUIREMENTS.md) and [coordinator contract](runtime/mod.md) describe
the lifecycle. Behavior and literal fixtures use
[CR2SE d40b6278f24430e814d7fa82094f57261bfcac9f](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f).
The coordinator owns local decision/result lifetime; families own their effects;
Ledger owns exact balances/payment. This separation permits recovery without
reading private dependency state or claiming atomic cross-peer settlement.

## Runtime ownership and limits

[Authority](authority.md) is local, identity/action/byte-scoped and revocable;
[Context](contract.md) checks version 1, cancellation and monotonic deadlines.
Trusted composition supplies authenticated Peer context and explicit local requester
input; remote descriptions cannot install handlers, select a payer or grant spending.
Registry requires exact understood definitions and an independent catalogue for
reserved standard names. [Publication composition](../../composition/services.md)
unpublishes before disabling new selection and retains accepted recovery pins.

The coordinator retains consent before one original-key Ledger reservation,
validates complete owned input, journals an execution marker before dispatch,
retains complete validated results, prepares exact handler/Ledger effects and
records one immutable commit or abort decision. It exposes success only after
required participants acknowledge delivery. Status/resume replay decisions rather
than execution. Generic requester work with a possible remote commitment remains
Unknown after reply loss; it is never automatically resubmitted or refunded.

Defaults are 4096 handlers and retained operations, 65536 logical nodes and byte
bodies, 64 unaccepted quotes, and 16777216 state bytes. Body quota includes each
retained input/output byte value plus every complete result buffer. Whole-image
byte/node quotas also bound metadata, opaque tokens and pins. Capacity exhaustion
refuses new work; terminal history remains a lifetime replay guard. This version
provides no compaction/deletion procedure or automatic migration. Lower limits
refuse reopening without changing stored bytes. The private coordinator payload is
version 2; old alpha payload 1 is Unsupported, while the local call/adapter contract
and protected outer framing remain version 1.

## Results, payment and checks

Complete Blob/result values belong to the coordinator and survive caller-session
expiration. The [local serialized adapter](adapter.md) preserves integer widths,
IEEE bits, exact byte values and explicit authority, but defines no peer wire
protocol or standalone IPC transport. Returned copies have finite limits; losing
a reply never proves rollback or authorizes repeating execution.

Service commitment, remote receipt, property verification and payment are separate
observations. Third-party payment requests/receipt confirmations use original
Ledger contracts; unknown or refused payment does not erase completed service
work. Requester-local verification receives private expectations without network,
refund or trust changes. Paid evidence checks and existing included allowances
have separate admission/timing rules; zero price is permitted only for proven
finite included checks, never ordinary work.

[Issue #10](https://github.com/beothorn/CR2SE/issues/10) records an unresolved protocol
ambiguity after a provider commits evidence but the response is lost. Current
handling keeps requester-local timeout uncharged and preserves an already committed
provider charge; it does not promise a globally uncharged timeout or reconcile
balances automatically. This limitation is explicit in the
[acceptance matrix](ACCEPTANCE.md) and must be revisited for paid-check networking.

## Peer Protocol handoff

Local Services scope is complete under the [34-step plan](../../../plans/7_Services_module/PLAN.md)
and its qualified acceptance evidence. Next supply canonical peer records/vectors,
detached-body assembly, exact quote encoding/hash association, authenticated
original-peer events, explicit result/end/failure classification and issuer controls.
No network application protocol, paid Storage provider, Computation runtime,
Discovery index or feed provider is installed by this module. The custom example
and test-only family contracts do not imply full standard-family support or
minimum-node conformance. Ordinary unknown outcomes must retain obligations;
network disconnect must never trigger automatic execution retry.
