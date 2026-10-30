# Protected Identity lifecycle provider

Identity.new(store, entropy, limits, version) composes explicit local dependencies,
requires contract version one and room for a complete credential record, and
creates a fresh local authority domain. It performs no key generation or network
registration. The store implements protected CredentialRecord persistence.

create first checks store administration, then generates independent signing and
persistent encryption pairs, derives the exact ID and binding, stages a complete
record, and prepares all fallible public output before committing. Only durable
installation success returns an opaque IdentityHandle and PublicCredential.
Failures release acquired staging, preserve their typed category and never retry
or silently replace keys. OutcomeUnknown requires store reconciliation, not
another identity creation. Loading remains explicitly unavailable until implemented.

IdentityHandle has no diagnostic, serialization or raw key access. It is bound
to this provider's domain and retains private authority with scoped cleanup.
grant requires store administration, a matching handle and a positive byte budget;
it issues a purpose-typed capability for that identity. public requires ReadPublic
and matching handle/domain, revalidates the retained association and returns only
public values. Foreign, expired, revoked and wrong-identity authority is rejected.
Operations must also enforce their configured policy limits when implemented.

Creation grants no network authentication, trust, credits or service state.
The provider relies on the store's durable contract and does not pretend that
test-only persistence is production storage; the Linux FileStore supplies it.
