# Transfer fee negotiation

Quotes is a finite transient registry for one local issuer/authority. Peer is
trusted authenticated owner/connection/stream context supplied by the adapter,
never peer JSON. Request carries a signed order, owner key, maximum fee, explicit
local policy fee and a finite monotonic expiry. Quote checks proof, fee<amount,
fee<=maximum and expiry before admission; zero fee is allowed. Expired quotes are
reclaimed. Capacity and duplicate live-stream reuse fail without accounting effects.

Accept requires Control authority, the original opaque Quote, identical Peer and
exact fee before expiry. It consumes the live quote and returns an opaque Accepted
association. Discard frees disconnected unaccepted exchanges. Restart loses quotes,
not committed transfer records. Accepted offers public order/fee observations and
receiver-domain validation, never the privileged Authority issuer. The Ledger must
recheck available funds and permanent replay under its writer before committing.
Network composition recognizes accepted signed retries before entering this gate.
