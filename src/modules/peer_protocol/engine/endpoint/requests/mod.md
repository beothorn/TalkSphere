# Explicit outgoing requests

`Endpoint.request(Plan)` prepares a one-shot factory and a capacity-one completion
receiver. Preparation opens no stream and causes no remote or accounting effects.
Pass the factory to `Bridge.open_with` on the intended authenticated connection.
The factory consumes its intent exactly once, receives actual endpoint/connection/
stream scope, and validates local owner and opener before constructing controls.
Reusing the same factory is a conflict, not a second financial operation.

Plans are trusted local objects, never peer-deserialized commands:

- `Board` retrieves metadata into an explicit direct-publisher cache.
- `Definition` supplies that cache and an exact offering ID after retrieving Board.
- `Invoke` supplies a sealed selection, local input, spending and separate consent.
- `Balance` queries the authenticated caller's original-issuer balance.
- `TransferGet` supplies original owner and16-byte lifetime transfer ID.
- `Transfer` supplies the canonical signed order/public proof and separate fee policy.
- `Claim` supplies only the exact encrypted object and expected public32-byte claim ID.

`FeeConsent.approve` receives an independently validated exact original issuer offer.
`ExactFee` approves only its explicit integer-credit fee, including zero; it never
silently substitutes a different fee under the signed maximum. Stored original
transfer retries return a historical result without inventing fresh fee consent.
No private signer or clear bearer secret enters a request plan or diagnostic.

`Reply` contains a direct metadata receipt, a Ledger query observation, an opaque
original-issuer transfer completion, a public redeemed claim or the original local
Services key for committed ordinary work. Work commitment is distinct from any
outstanding third-party payment. Controls do not blindly overwrite owned credits.

`Completion.scope` identifies the actual original authenticated wire exchange.
`result` publishes success only after both permitted ENDs, actual normal Transport
closure and the corresponding module's full validation/commit gate. Interruption
is Unknown, even when partial data was available. `failure` preserves the exact
bounded remote code/message as untrusted diagnostics; `cause` preserves a local
callback refusal. Neither overrides uncertainty or authorizes rollback/retry.

The completion channel contains at most one observation. Dropping the receiver
never retries work, changes accounting or removes original durable records.
Constructor failure returns a local error without an opening control; failures
after accepted dispatch remain attached to their original operation owners.
See [ordinary invocation consent](invocation.md) and [direct cache](../cache.md).
