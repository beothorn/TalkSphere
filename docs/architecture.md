# TalkSphere architecture

TalkSphere is designed as a decentralized peer-to-peer system where peers exchange resources, earn peer-specific credits, and use trust relationships to make communication and storage economically meaningful.

## Current Linux runtime

The current Linux implementation is a small command-line runtime organized by responsibility. Each responsibility is isolated as much as possible:

- **Startup**: `main.c` connects the modules. It parses commands, prepares local storage when needed, delegates existing domain operations, prints placeholders for missing command bodies, and starts networking for `run`.
- **Argument parsing**: validates user input and maps domain commands such as `run`, `config`, `encryption`, `ledger`, `network`, `offerings`, and `share` into program modes.
- **Files**: resolves and creates local storage, including the local identifier and ledger directory.
- **Ledger**: records local credit balances as files and calculates owned/owed totals.
- **Network**: runs a TCP listener and sends outbound TCP messages.
- **Message parsing**: interprets protocol text and applies ledger effects.
- **Encryption**: reserves the API boundary for future key creation, encryption, and signing. The current functions are placeholders and do not perform cryptography yet.

## Runtime flow

1. The process starts in `main.c`.
2. Program arguments are parsed into a command mode and optional command data such as ports, message text, or peer addresses.
3. The storage path is resolved, and required app files are created only for commands that need local files.
4. In `ledger credit_summary`, the ledger module reads balances and prints owned/owed totals.
5. In `run`, the network module listens for TCP messages.
6. Received messages are passed to the message parsing module.
7. Message parsing updates the ledger or prints delivered message text depending on the message command.

## Module boundaries

TalkSphere should prefer domain-oriented modules over technology-oriented layers. Each module owns its own concepts and should avoid reaching into another module's internal details.

- Network code moves bytes and owns socket behavior.
- Message parsing owns protocol interpretation.
- Ledger code owns credit accounting.
- File code owns storage bootstrapping.
- Encryption code will own cryptographic operations behind byte-oriented interfaces.

Shared code should remain minimal. A shared contract is appropriate only when multiple unrelated modules truly need it.

## Future architecture direction

The project notes describe a broader system with identities, signed messages, encrypted payloads, trust-based reachability, peer-specific credits, and resource-sharing challenges. The Linux runtime can grow toward that design by keeping these boundaries clear:

- Add real key generation, encryption, and signing inside the encryption module.
- Keep signed/encrypted payload formats outside raw socket transport code.
- Add resource-sharing modules for storage or computation without coupling them directly to ledger internals.
- Treat identities and keys as application-level concepts that can be reused by messaging, dashboard, and discovery applications.
