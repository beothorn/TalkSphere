# Provider quote construction

Issuer binds one local identity to explicit Peer Protocol and Services authority,
finite limits, an economic Gate and an Entropy port. SystemEntropy uses the OS
CSPRNG; unavailable randomness fails without a fallback identifier.

issue consumes a provided invocation Proposal and assesses its bounded declared
metadata through the installed provider. It checks current terms before and after
assessment, independently calculates the exact family price and verifies price
plus any preagreed issuer fee against the original spending ceiling. Capacity is
observed without allocating a hold. Unsupported check purposes or provider factors
fail before acceptance or execution.

Only specified provider factors appear in pricingInputs: file manifest/download
sizes, Storage renewal size and Messaging recovery/renewal size. Messaging removal
binds releaseCredits separately. Other operations require an empty factor object.
Ordinary quotes contain fresh32-byte quoteId, complete offering, exact issuer,
fixed/metered mode and net maximumPrice; preagreed transfer fields are retained.
The original request payload is never copied into a quote.

Issued owns the immutable exact JSON, its SHA256 hash, random ID, original
Association, Price, provisional Pending input, Payment and local monotonic expiry.
Expiry is clipped to both caller deadlines. The Services slot retains the same
price; inconsistent family callbacks fail and release that exact slot. Complete
control size and caller byte grants are checked before returning a quote.

No quote reserves money or authorizes dispatch. discard accepts only the original
issuer's opaque token and scoped contexts and releases its unaccepted slot. A
handle cannot release a replacement generation or undo accepted work. Composition
must discard abandoned unaccepted quotes and separately retain accepted outcomes.
