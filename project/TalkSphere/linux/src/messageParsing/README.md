# Message parsing module

This module owns interpretation of incoming protocol text.

## Responsibilities

- Parse received message text.
- Handle payment messages.
- Handle user message delivery.
- Return local offerings when peers request `LIST_OFFERINGS`.
- Forward local client offering requests to the configured connected peer.
- Forward local client message requests to the configured connected peer.
- Coordinate ledger updates required by message actions.
- Delegate outbound sending through injected message-processing dependencies.

## Boundaries

This module should understand message intent, but it should not own socket setup or raw filesystem bootstrapping.
