# Private requester byte-check verification

`verify` consumes the original accepted Services check invocation, original public
challenge, authenticated original reply, optional independently retained Expected,
requester-local Activity, original transport Observations and value budgets.
It samples no current clock and does not mutate storage, payments or local trust.
Private expectations are a separate native type, never fields inferred from or
sent in the challenge/evidence value.

`Expected` contains the original lease ID, zero-based offset and byte from the
requester's own stored data (ciphertext when ciphertext was uploaded).
`Activity.Active` means trusted lease metadata and local clock policy establish
original activity without expiration ambiguity. Inactive or Ambiguous yields
inconclusive; a remote timestamp or generic failure cannot establish Active.

`Observations.sent` is captured immediately after complete original challenge
transmission; received is the original complete evidence/report arrival; ended
is the original normal END observation. All must exist on the same trusted local
monotonic epoch, in order, with complete end minus send no greater than the
accepted response interval. Equality is timely; one millisecond later is not.
Missing stamps, abnormal completion, incomparable clocks or reversed observations
are inconclusive. A delayed worker uses these saved observations and cannot
restart a timer. Use original Peer Protocol input/evidence/end observations and
its timestamp-preserving send/finish APIs, not provider JSON or callback-run time.

`Reply.Evidence` carries authenticated provider identity and complete evidence.
After original contract/price/challenge, identity/association, timing and private
expectation checks, equal bytes produce pass; a well-formed wrong byte produces
fail. Malformed, foreign, mismatched, incomplete or provider-outcome-bearing values
are inconclusive. `Reply.Unavailable` is reserved for an explicit authenticated
original report that the independently authorized active lease or requested byte
is unavailable; with reliable activity/timing it produces fail. Generic internal,
capacity, policy, authentication, transport or timeout failures are Reply.Missing
and inconclusive. The peer adapter must preserve that distinction instead of
turning every unavailable-looking failure into storage evidence.

No expected byte gives inconclusive, including after a provider refusal. Returned
outcomes neither settle nor undo a payment. Preserve requester and provider
Services/Ledger observations separately: [CR2SE issue #10](https://github.com/beothorn/CR2SE/issues/10)
still allows a provider commitment before the requester loses the response.
Inconclusive is not evidence of zero charge and never implies automatic refund,
trust change, retransmission or storage repair.

Run `cargo test --locked modules::storage::requester::byte_check` from the repository
root. Optional [secure sampling](sampling.md) chooses fresh unbiased offsets.
Rules follow pinned [Storage section 10](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md#10-storage-check).
