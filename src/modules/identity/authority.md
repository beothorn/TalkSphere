# Local identity authority

Authority.new/default creates a trusted local issuance domain. Possession of the
issuer grants administrative power; composition must not give it to peer handlers
or derive it from payload metadata. grant issues an opaque Capability of the
specified purpose, bound to one binary identity and a positive byte ceiling.
administration issues the distinct unbound Administration capability needed before
creation/loading discovers an ID. Zero budgets fail; revoked issuers deny grants.

Purpose markers distinguish Administration, ReadPublic, Sign, Decrypt, Backup and
Import. Consumers fix the purpose in their own method signatures. Capabilities
have no raw constructor, serialization, diagnostic representation or purpose
conversion. They contain weak lifetime references, not persistent bearer secrets.

check validates the issuing domain, active lifetime, identity and byte budget;
check_administration additionally requires unbound Administration. Foreign,
revoked, expired and wrong-identity capabilities fail with AccessDenied before
resource or existence details. Excess bytes return ResourceLimit. revoke denies
all current and future grants in that domain without changing durable state;
dropping the issuer also invalidates existing capabilities. No network or trust
operation occurs. These are in-process contracts, not remote access tokens.
