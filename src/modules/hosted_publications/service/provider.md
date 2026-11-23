# Service provider

The Services provider adapter: it maps Services' stage, prepare, finalize and commit calls onto the participant and the provider engine.

- `ServiceProvider::new(participant, budget, maximumBytes)`: binds the participant to the value bounds Services uses for results. `Unavailable` until the participant has completed recovery, so nothing is served (or registered)
  over unrecovered state; `Invalid` for a zero bound.
- `assess` / `assess_metadata`: validate a request and describe it before any body is received. Requests are served only when the offering is provided, its credit issuer is the local identity and the
  requester is the authenticated remote (otherwise `Denied`). A store is assessed from its header (a zero-filled placeholder of the declared length stands in for the content, and only the header rules,
  retention, room, quota and price are judged) and projects to `{operation, contentSize, retentionSeconds}`; other operations project to their complete request. A get carries the exact `contentSize` the
  provider would return as its factor (an unreadable publication is concealed as `Denied`; a size above the requester's `maximumBytes` is `Limit`); a renewal carries the placement's `contentSize`; a removal
  carries the quoted `releaseCredits` (the recorded credits for a replay). A list limit above the offering or a resource kind is `Invalid`.
- `execute`: validates the full request (signature included), builds its effect and journals it as prepared (everything it touches stays locked), then returns the staged *template* completion. A repeated call for
  the same key returns the same staged result. List and get keep their encoded request in the journal and are read live.
- `completion`: rebuilds the staged completion from the journal without doing the work again; `Unknown` when nothing is staged.
- `prepare`: confirms the supplied effect bytes are exactly the staged effect and returns the participant's token.
- Finalization (stores only): `finalize` samples the decision time and fills the template (acceptance and expiry) without applying anything; `commit_finalized` commits at exactly that time and checks that the outcome
  rebuilds the finalized result; `validate_template` accepts a template only when its effect belongs to the request and its output has exactly the promised shape (reading no live state);
  `validate_local_result` accepts results that differ from the staged template in the decision-time fields alone.
- `commit`, `abort`, `status`: ordinary decision delivery and durable participant state.
- `allow_cached_retry`: true only when the same authenticated publisher repeats a removal whose removal ID is already recorded for the placement.
- `evidence`: one authorized content byte for the publisher of a pending placement.
