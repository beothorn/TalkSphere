# Messaging errors

`Invalid` (malformed or forbidden), `Unsupported`, `NotAvailable` (unauthorized and unknown placements are the same), `Limit`,
`Conflict` (stale expiry, conflicting replay ID), `Insufficient`, `Storage` (failed before commitment), `Unknown` (may have committed:
recover through status, never retry blindly) and `Unavailable` (clock, entropy or signer missing). A failure before durable commitment is
never charged. Mapping to Services: each maps to the Services error of the same meaning, `NotAvailable` to `Denied`; Services errors map back
conservatively, `Cancelled`, `EvidenceUnavailable` and `Unknown` becoming `Unknown`. Errors carry no identifiers, payloads or peer text.
