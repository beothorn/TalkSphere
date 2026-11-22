# Service provider

The Services provider adapter: it maps Services' stage, prepare, finalize and commit calls onto the provider engine.

- `ServiceProvider::new(provider, budget, maximum_bytes)`: binds an engine to the value bounds Services uses for results. `Unavailable` until the participant has completed recovery, so nothing is served (or registered) over unrecovered state; `Invalid` for a zero bound.
- `assess` / `assess_metadata`: validate a request and describe it before any body is received. A store is assessed from its header (the declared payload length stands in for the body) and its projection is `{operation, payloadSize, retentionSeconds}`; other operations project to their complete request. Recoveries and renewals carry the placement's `payloadSize` as their provider factor, removals carry the quoted `releaseCredits`. The staging reservation taken for a store is released at once because the body has not arrived.
- `execute`: validates the full request, builds its effect and journals it as prepared (the placement stays locked), then returns the staged *template* completion. A repeated call for the same key returns the same staged result. Read-only operations (`discover`, `status`) keep their encoded request in the journal.
- `completion`: rebuilds the staged completion from the journal without doing the work again; `Unknown` when nothing is staged.
- `prepare`: confirms the supplied effect bytes are exactly the staged effect and returns the participant's token.
- Finalization (stores and recoveries): `finalize` samples the decision time and fills the template (acceptance and expiry for a store, the provider-signed receipt for a recovery) without applying anything; `commit_finalized` commits at exactly that time and checks that the outcome rebuilds the finalized result; `validate_template` and `validate_local_result` accept only the staged template and results that differ from it in the decision-time fields alone.
- `commit`, `abort`, `status`: ordinary decision delivery and durable participant state.
- `allow_cached_retry`: true only when the same authenticated caller repeats a recovery (or removal) whose recovery (or removal) ID is already recorded for the placement.
- `evidence`: one authorized payload byte for the sender of a pending placement.
