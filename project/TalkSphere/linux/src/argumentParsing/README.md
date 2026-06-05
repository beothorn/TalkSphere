# Argument parsing module

This module validates the command-line interface for the Linux runtime.

## Responsibilities

- Parse optional client and server ports.
- Parse the optional application storage directory used as this instance's home folder.
- Detect the `--id` mode.
- Detect the `--home` mode.
- Detect the `--ledger-summary` mode.
- Detect the `--help` mode.
- Reject invalid ports, unsupported argument counts, and conflicting client/server ports.
- Print usage text when the command shape is invalid.

## Boundaries

This module should only decide what the user asked the program to do. It should not create files, open sockets, or process messages.

## Server Command Shape

The server mode accepts ports and an optional home folder:

```bash
build/talksphere <listen_port> <peer_port> [home_folder]
```

Two local instances should use different `listen_port` values and different `home_folder` values so their local identities, offerings, and ledgers stay separate.
