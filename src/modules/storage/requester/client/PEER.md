# Requester Peer Protocol integration evidence

The installed Client runs through public Board Cache/Selection, Peer Protocol
request/quote/consent and result-receiver contracts, actual requester Services
coordination and a real Ledger participant. No extra Storage wire encoding or
cached-retry operation is introduced. The existing Peer Protocol client owns the
original authorized input and detached sources until exact consent and reservation.

The integration tests cover:

- Store, retrieve, renew and remove proposals and exact current quoted prices.
- Provider-size, issuer, price and contract tampering rejected before consent.
- Changed supported economic terms requiring a fresh exact-byte quote hash.
- Explicit finite provider-issued test funding and original-key local holds.
- Only complete result bodies, result.end and normal original END reaching Services.
- Independent Storage result validation before requester preparation/commit.
- One exact requester-side local charge despite repeated commit delivery.
- Missing terminal markers, abnormal END or corrupt bytes retaining unresolved
  original holds without exposing successful application output.
- Detached 5000-byte store sources and retrieval results, bounded queue framing,
  original body offsets and last-byte completion.
- Separately paid check proposals containing only public challenge fields,
  three-credit consent/reservation and refusal of leaked private expected bytes.
- No Storage opt-in to Messaging cached retries.

The tests deliberately use controlled remote quotes/results and a controlled
current-quote port. Transport provenance is supplied by the trusted test adapter;
this is not an authenticated native-process test or evidence that an actual
provider retained bytes. The requester coordinator and Ledger use explicit
volatile Memory adapters here. They establish orchestration and local accounting
behavior, not disk restart durability or provider-side accounting. Those remain
separate plan steps 31–35. No result here resolves the acceptance-time conflict
in CR2SE issue #11 or completes provider registration/publication in step 28.

From the repository root run:

```sh
cargo test --locked modules::storage::requester::client::tests::wire
```

The ordinary and check Peer Protocol APIs continue enforcing distinct local
Request/Invoke/Coordinate grants, immutable original association, selected issuer,
explicit spending bounds and exact consent hashes. No private expected data,
encryption key or application trust decision is put into public challenge input.
