# Argument parsing module

This module validates the command-line interface for the Linux runtime.

## Responsibilities

- Parse optional client and server ports.
- Parse the optional application storage directory.
- Detect the `--ledger-summary` mode.
- Reject invalid ports, unsupported argument counts, and conflicting client/server ports.
- Print usage text when the command shape is invalid.

## Boundaries

This module should only decide what the user asked the program to do. It should not create files, open sockets, or process messages.
