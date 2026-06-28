# Argument parsing module

This module validates the command-line interface for the Linux runtime.

## Responsibilities

- Parse the global dry-run flag, `--dry-run` or `d`.
- Parse the global home directory override, `-d <home_folder>` or `--directory-home <home_folder>`.
- Parse main help, `--help`, `help`, or `h`.
- Parse `run <listen_port> <peer_port>`.
- Parse `config get home` and `files home`.
- Parse encryption commands and encryption-specific help.
- Parse ledger commands and ledger-specific help.
- Parse network, offerings, local talk, and shared-storage command shapes.
- Reject invalid ports, unsupported argument counts, and conflicting listen/peer ports.
- Print help text when the command shape is invalid or when help is requested.

## Boundaries

This module should only decide what the user asked the program to do. It should not create files, open sockets, process messages, or implement command behavior.

## Command Shape

The command line accepts global options followed by a domain command:

```bash
talksphere [--dry-run|d] [-d|--directory-home <home_folder>] <command> [arguments]
talksphere [--help|help|h]
```

If no home folder is passed, the application uses its default storage resolution. If `-d` or `--directory-home` is passed, every command uses that folder instead:

```bash
talksphere run <listen_port> <peer_port>
talksphere -d /tmp/Alice run <listen_port> <peer_port>
talksphere --directory-home /tmp/Alice run <listen_port> <peer_port>
talksphere offerings get
talksphere -d /tmp/Alice offerings get
```

Two local instances should use different `listen_port` values and different `home_folder` values so their local identities, offerings, and ledgers stay separate.

The `talk` command accepts a local client port and a child command:

```bash
talksphere talk -p <client_port> offerings
talksphere talk -p <client_port> message "message"
```

This shape talks to an already-running local instance instead of reading local files directly.
