# Ledger

The ledger is the local credit store for one TalkSphere installation.
It is intentionally simple so the current behavior is easy to inspect and discuss before we make the design more durable.

## Files on disk

The application storage directory contains a `ledger` folder next to the local `id` file:

```text
<storage_directory>/
  id
  ledger/
    <identifier-a>
    <identifier-b>
```

Each file inside `ledger` is named after an identifier.
The content of each file is a decimal integer with the number of credits recorded for that identifier on this installation.

## Updating credits

- `PAY:<ID>` adds one credit to `ledger/<ID>` on the instance that receives the command.
- `MESSAGE:<text>` reads the local installation `id`, spends one credit from `ledger/<local-id>`, and only prints the message when that spend succeeds.
- `CREDITS:<ID>` reads `ledger/<ID>` and prints the current balance for that identifier.
- Missing ledger files count as `0` credits.

## Owned and owed totals

The ledger can now summarize balances from the point of view of the local installation id:

- **Owned credits** are the credits stored under `ledger/<local-id>`.
  These are the credits this installation can spend for behavior that checks the local id.
- **Owed credits** are the sum of credits stored under every other identifier in the local ledger directory.
  These represent credits this local installation has recorded for other ids.

You can print those totals with:

```bash
./talksphere --ledger-summary [storage_directory]
```

When `storage_directory` is omitted, TalkSphere resolves the same default application data directory used by the server mode.
