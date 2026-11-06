# Exact accounting values

Trust is an exact local integer in millionths, inclusive 0..1000000; construction
rejects values outside that range and unknown peers default to zero. It has no
wire representation and never changes from authentication or balance observation.

Accounts owns independent maps for credits owned by issuer, local credits issued
to an owner, and local trust. Default is empty. View requires current local Read
authority and returns copied owned/issued amounts and trust for exactly one peer.
Remote balance requires Control authority and reports only issuance for the
trusted adapter's authenticated caller. The adapter must obtain that identity
from authenticated Transport context, never payload metadata. Both reject local
self-balances and preserve all state. Missing entries are zero, never an aggregate
of currencies. Amounts are exact uint64 including its maximum.
