# Identity mod contract

Identity exports api, immutable config, structured errors and explicit dependency ports. Its callers are authorized local administration, Transport and future Ledger/application adapters. Cryptography owns algorithms; the platform owns protected persistence. No listener or identity is created on import.

The id module derives and validates binary identities; crypto maps the public real
Provider into the injected Cryptography contract.
