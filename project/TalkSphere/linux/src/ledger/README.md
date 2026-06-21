# Ledger module

This module owns TalkSphere credit accounting for the Linux runtime.

## Responsibilities

- Store per-peer credit balances on disk.
- Add credits when a peer pays the local identity.
- Spend credits when the local identity consumes a paid action.
- Summarize owned and owed credits.
- Own ledger command behavior in `ledger_application.c`.

## Boundaries

This module should manage ledger files, credit arithmetic, and ledger-specific display only. Message parsing decides when ledger actions are requested, and the files module decides where app storage lives.
