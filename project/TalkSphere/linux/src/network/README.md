# Network module

This module owns the Linux TCP socket channel.

## Responsibilities

- Create TCP sockets.
- Bind the local listening port.
- Accept incoming connections.
- Read received bytes into message text.
- Send outgoing message text to remote endpoints.
- Send command text to a local instance and collect its response for CLI commands.
- Own network command behavior in `network_application.c`.
- Delegate parsed message behavior to the message parsing module.

## Boundaries

This module should move bytes between peers. It should not decide ledger policy, interpret offerings, or know how storage files are organized beyond passing the configured app storage directory to downstream modules.
