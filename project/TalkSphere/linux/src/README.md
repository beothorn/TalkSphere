# Linux source modules

This folder contains the Linux implementation of the TalkSphere command-line runtime.

## Responsibilities

- `main.c` wires startup together: it parses arguments, resolves storage, ensures local files exist, prints ledger summaries when requested, and otherwise starts the socket channel.
- `argumentParsing` owns command-line parsing and validation.
- `common` contains small shared contracts that are safe for unrelated modules to depend on.
- `encryption` owns the future cryptographic boundary for key creation, encryption, and signing.
- `files` owns local application storage setup and local identity file handling.
- `ledger` owns credit accounting on disk.
- `messageParsing` owns incoming message interpretation and dispatch.
- `network` owns TCP socket setup, listening, receiving, and sending.

## Design notes

Modules should keep their own domain concepts private. Shared files should stay small and only contain contracts that are genuinely needed by multiple domains.
