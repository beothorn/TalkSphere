# Credit withdraw module

This module owns credit withdraw code storage and command behavior.

## Responsibilities

- Store withdraw codes for the local identity.
- Remove stored withdraw codes.
- List stored withdraw codes.
- Own credit withdraw command behavior in `credit_withdraw_application.c`.

## Boundaries

This module should manage credit withdraw code records only. Ledger balance accounting stays in the ledger module, and local identity file storage stays in the files module.
