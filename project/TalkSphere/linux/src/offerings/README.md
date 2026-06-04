# Offerings module

This module owns reading the local offerings document.

## Responsibilities

- Load the local offerings text from the app storage directory.
- Return the offerings text as an opaque protocol document.
- Fail when the caller gives no output buffer or when the buffer cannot hold the whole document.

## Current default document

The file describes an entity that says it is always reachable at `www.isageek.com.br:9876`.
It buys storage for its own credits, paying `1` credit for `100000` size units kept for `10` days.
It sells storage for its own credits, charging `1` credit for `100000` size units kept for `30` days.
It also sells `storeMessage`, which is storage for signed encrypted messages that a second party can recover later.
The current `storeMessage` offer stores `1` size unit for `10` days for `0.01` credit.
Finally, it sells `askForMessages` for `0.001` credit so another entity can check whether pending messages exist.

## Boundaries

This module should not create application files, parse offer semantics, charge credits, or open network sockets.
The files module creates the default document, and this module only reads what is already present.
