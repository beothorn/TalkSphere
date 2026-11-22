# Release

`LedgerIssuer(ledger, grant, timeout)` is the participant's release credit issuer backed by a Ledger. `issue(key, recipient, credits)` calls the Ledger's once-per-operation `issue_release` with the supplied Coordinate grant and a deadline `timeout` from now. Ledger failures map to the Messaging error with the same meaning (`Denied`, `Cancelled` and `Unavailable` become `Unavailable`).
