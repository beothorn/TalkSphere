# Linux source modules

This folder contains the Linux implementation of the TalkSphere command-line runtime.

## Responsibilities

- `main.c` wires startup together: it parses commands, resolves storage, ensures local files exist when needed, delegates implemented domain commands, prints placeholders for missing command bodies, asks running local instances for peer offerings and peer message sending, and starts the socket channel for `run`.
- `argumentParsing` owns command-line parsing and validation.
- `common` contains small shared contracts that are safe for unrelated modules to depend on.
- `encryption` owns the future cryptographic boundary for key creation, encryption, and signing.
- `files` owns local application storage setup and local identity file handling.
- `ledger` owns credit accounting on disk and ledger summary display.
- `messageParsing` owns incoming message interpretation, local control messages, peer offering forwarding, peer message forwarding, and dispatch.
- `network` owns TCP socket setup, listening, receiving, sending, and request/response socket calls.
- `sharedStorage` owns future lifecycle rules for shared storage, recovered sold storage, and age-based cleanup.

## Design notes

Modules should keep their own domain concepts private. Shared files should stay small and only contain contracts that are genuinely needed by multiple domains.
