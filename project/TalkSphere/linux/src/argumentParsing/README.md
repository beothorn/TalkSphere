# Argument parsing module

This module validates the command-line interface for the Linux runtime.

## Responsibilities

- Parse the global dry-run flag, `--dry-run` or `d`.
- Parse main help, `--help`, `help`, or `h`.
- Parse `run <listen_port> <peer_port> [home_folder]`.
- Parse `config get home` and `files home`.
- Parse encryption commands and encryption-specific help.
- Parse ledger commands and ledger-specific help.
- Parse network, offerings, and shared-storage command shapes.
- Reject invalid ports, unsupported argument counts, and conflicting listen/peer ports.
- Print help text when the command shape is invalid or when help is requested.

## Boundaries

This module should only decide what the user asked the program to do. It should not create files, open sockets, process messages, or implement command behavior.

## Command Shape

The command line accepts a global dry-run flag followed by a domain command:

```bash
build/talksphere [--dry-run|d] <command> [arguments]
build/talksphere [--help|help|h]
```

The `run` command accepts ports and an optional home folder:

```bash
build/talksphere run <listen_port> <peer_port> [home_folder]
```

Two local instances should use different `listen_port` values and different `home_folder` values so their local identities, offerings, and ledgers stay separate.
