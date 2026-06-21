# Files module

This module owns local application file and directory setup.

## Responsibilities

- Resolve the app storage directory.
- Create the application directory when it is missing.
- Create the ledger directory when it is missing.
- Create and read the local identifier file.
- Create the default local offerings file when it is missing.
- Own file command behavior in `files_application.c`.

## Boundaries

This module should only manage filesystem paths and local app files. It should not interpret offerings, messages, maintain credit balances, or open network sockets.
