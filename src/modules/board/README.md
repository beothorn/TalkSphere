# Board

Board owns atomic offering/definition snapshots, attributed remote caches and
selection. Current status: requirements inventory and initial authority/resource
contracts plus deterministic test helpers; snapshot reads explicitly return Unsupported. Publication,
persistence, validation, caches and selection are still pending in the
[plan](../../../plans/5_Board_module/PLAN.md).

Callers are local administrators, future Services and Peer Control. Composition
supplies trusted authority, persistence and semantic capabilities. Identity IDs
are reused; Services schema formats will be validated without its runtime. Board
never owns balances, accepted invocations or Transport internals. Read authority
does not authorize spending or execution. No Board network service is registered.

See [upstream Board](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/board)
and the [acceptance inventory](ACCEPTANCE.md). From the repository root:

```sh
cargo build --locked
cargo test --locked modules::board
cargo run --locked -- --help
```

There is no Board CLI command yet. Exercise the direct-call scaffold through its
unit tests. See [authority](authority.md), [limits](config.md) and
[boundary contracts](contract.md).

## Local validation and inspection

```sh
cargo run --locked -- board-validate --board-file board.json
cargo run --locked -- help --board-file
cargo run --locked -- board-show --board-store board-state --publisher CR2SE_ID
cargo run --locked -- help --board-store
```

Validation checks bounded Board syntax/structure and exact terms, reporting that
definitions and local publishability are not established. Inspection requires an
existing protected Linux store and exact expected publisher; it validates all
stored definition associations before printing original public Board JSON.
No command publishes capabilities, registers services or spends. Library callers
with real capabilities exercise publication through Provider; the application
still registers no Peer Protocol or fake service handlers.
