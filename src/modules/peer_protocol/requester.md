# Requester quotes and spending consent

Client owns one local identity, separate Peer Protocol and Services authority,
an explicitly installed requester Registry, finite limits and an opaque domain.
It does not require or install a provider handler.

Spending contains the original total maximum and Payment (optional third-party
transfer and exact fee). request requires outgoing Request authority, a trusted
Association, retrieved Selection, complete locally authorized input and Spending.
It validates the exact installed contract, logical input, roles and issuer choice
before encoding a bounded proposal. Request retains original input, deadlines,
terms and immutable proposal bytes. Detached sources are not sent before acceptance.

validate requires separate incoming Invoke authority and Services Request authority.
It consumes only this Client's Request, requires the same authenticated association
and checks the quote against the retrieved contract. Changed identifiers, version
or description require fresh discovery. Supported economic changes can become a
candidate; they are never accepted automatically. The issuer and optional payment
must equal the original proposal. Specified provider factors are passed through
independent installed family validation. Unknown factors and unsolicited release
credits cannot alter ordinary work.

The installed requester independently derives metadata from its original input,
checks factors, recalculates fixed or permitted Computation metering and compares
priceMode and maximumPrice exactly. Net price plus fee must fit uint64 and the
original total bound. Candidate exposes original quote bytes, SHA256 hash, quote
ID, price, selection, association, payment and retained local deadline. No JSON
normalization participates in the hash. No validation allocates credit holds or
executes a provider.

consent consumes a Candidate only with outgoing Request authority and an explicit
matching hash and total ceiling. Price plus fee must fit both the original and
consent ceilings; wrong ownership, cancellation or expired lifetime fails. Its
Consented result is approval for admission, not permission to send accept before
reserving capacity. Later admission must retain the exact quote and original
operation key, and a failed send after reservation remains potentially unknown.

reserve consumes this Client's Consented token under separate local Coordinate
and quote grants. It uses the acceptance contract to reserve, retain original
input and mark one outgoing attempt before returning accept bytes. Repeated
admission cannot double-dispatch; unknown local outcomes require original-key
recovery rather than a fresh wire attempt.
