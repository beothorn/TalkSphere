# Files module

This module owns local application file and directory setup.

## Responsibilities

- Resolve the app storage directory.
- Create the application directory when it is missing.
- Create the ledger directory when it is missing.
- Create and read the local identifier file.

## Boundaries

This module should only manage filesystem paths and local app files. It should not interpret messages, maintain credit balances, or open network sockets.
