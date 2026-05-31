# Shared storage module

This module owns the future shared-storage domain for the Linux runtime.

## Responsibilities

- Share local storage capacity with peers without mixing that policy into networking, ledger, or message parsing.
- Recover storage that was previously sold when that storage becomes available to the local app again.
- Track the age of shared storage so old shared data can be cleared after the configured time passes.

## Current placeholders

- `shared_storage_share_available_storage` validates that the caller provided an app storage root and currently performs no storage sharing yet.
- `shared_storage_recover_sold_storage` validates that the caller provided an app storage root and currently performs no recovery yet.
- `shared_storage_clear_aged_storage` validates that the caller provided an app storage root and rejects negative age limits before future cleanup logic exists.

## Boundaries

This module should deal only with storage-sharing lifecycle rules. It should not decide credit accounting, parse network messages, or create the base application files.
