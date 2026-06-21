# Linux source modules

This folder contains the Linux implementation of the TalkSphere command-line runtime.

## Responsibilities

- `main.c` only owns the process entrypoint and delegates startup to the application module.
- `application` owns lightweight command orchestration: it parses commands, resolves storage, ensures local files exist when needed, and delegates command behavior to the matching `*_application` file inside each module.
- `argumentParsing` owns command-line parsing and validation.
- `common` contains small shared contracts that are safe for unrelated modules to depend on.
- `creditWithdraw` owns credit withdraw code storage and its command-facing behavior.
- `encryption` owns the future cryptographic boundary for key creation, encryption, signing, and its command-facing behavior.
- `files` owns local application storage setup, local identity file handling, and file-related command-facing behavior.
- `ledger` owns credit accounting on disk, ledger summary display, and its command-facing behavior.
- `messageParsing` owns incoming message interpretation, local control messages, peer offering forwarding, peer message forwarding, and dispatch.
- `network` owns TCP socket setup, listening, receiving, sending, request/response socket calls, and network command-facing behavior.
- `offerings` owns local offerings reads and offerings command-facing behavior.
- `sharedStorage` owns future lifecycle rules for shared storage, recovered sold storage, age-based cleanup, and shared-storage command-facing behavior.

## Design notes

Modules should keep their own domain concepts private. Shared files should stay small and only contain contracts that are genuinely needed by multiple domains.
