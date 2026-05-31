# Message parsing module

This module owns interpretation of incoming protocol text.

## Responsibilities

- Parse received message text.
- Handle payment messages.
- Handle user message delivery.
- Coordinate ledger updates required by message actions.
- Delegate outbound sending through injected message-processing dependencies.

## Boundaries

This module should understand message intent, but it should not own socket setup or raw filesystem bootstrapping.
