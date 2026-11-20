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
- Requester-issued credits consuming finite issuance and recording one provider balance.
- External-issuer quotes binding the original transfer ID, fee and total consent bound.
- Committed external-currency work retaining its hold through unknown/refused
  payment until exact original issuer evidence settles once.
- Independent issuer accounting with a real owner signature, same-stream fee
  acceptance, exact gross/net balances and idempotent transfer commitment.
- Changed transfer IDs, recipients, gross amounts and fees rejected at settlement,
  with the original retained Storage result and local trust preserved.
- Only complete result bodies, result.end and normal original END reaching Services.
- Independent Storage result validation before requester preparation/commit.
- One exact requester-side local charge despite repeated commit delivery.
- Missing terminal markers, abnormal END or corrupt bytes retaining unresolved
  original holds without exposing successful application output.
- Detached 5000-byte store sources and retrieval results, bounded queue framing,
  original body offsets and last-byte completion.
- Separately paid check proposals containing only public challenge fields,
  three-credit consent/reservation and refusal of leaked private expected bytes.
- Wanted store against actual local Board publication with requester-owned input,
  exact consent, finite issuance and reversed provider-opened stream ordering.
- Wanted invitation role/ID mismatches, explicit application refusal and withdrawn
  terms before or during the policy callback leaving issuance untouched.
- No Storage opt-in to Messaging cached retries.

The tests deliberately use controlled remote quotes/results. Ordinary tests use
a controlled current-quote port; wanted tests query the actual local Board owner.
Transport provenance is supplied by the trusted test adapter;
this is not an authenticated native-process test or evidence that an actual
provider retained bytes. The ordinary quote/result, wanted and settlement tests use explicit volatile
Memory adapters for requester coordination/accounting. Separate
[native recovery tests](RECOVERY.md) now exercise independent Linux requester
Services/Ledger files, retained results and exact charging through reopen and
lost acknowledgements. Neither group establishes provider-side Storage durability.
The independent issuer ledger is still volatile; receipt provenance is supplied by the trusted test
adapter rather than an actual issuer Transport connection. Full provider payment
policies and persistence remain separate requirements in plan steps 31–35. No result here resolves the acceptance-time conflict
in CR2SE issue #11 or completes provider registration/publication in step 28.

From the repository root run:

```sh
cargo test --locked modules::storage::requester::client::tests::wire
```

The ordinary and check Peer Protocol APIs continue enforcing distinct local
Request/Invoke/Coordinate grants, immutable original association, selected issuer,
explicit spending bounds and exact consent hashes. No private expected data,
encryption key or application trust decision is put into public challenge input.

Wanted tests install only the publisher's requester. Invitations and successful
remote results are controlled fixtures; no prospective Storage provider runs.
They do not complete AT-E02's provider-side receipt/spend-back or demonstrate
follow-up operations against an actual newly created lease. Step 30 stays pending.
