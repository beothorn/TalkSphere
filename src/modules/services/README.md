# Services formats

Only the reusable schema-format validator is implemented for Board. Services
runtime remains a future milestone. Board is the current caller; future Services
and Peer Protocol will reuse these descriptions. There is no handler execution,
spending or network dependency. The format is specified by
[CR2SE Services](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/services/README.md).

From the repository root: cargo build --locked; cargo test --locked
modules::services; cargo run --locked -- --help. No Services command exists.
