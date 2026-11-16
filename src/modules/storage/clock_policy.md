# Requester clock policy

validate takes the provider's returned acceptance Unix seconds, an independently
observed requester UTC time and an authorized maximum difference in seconds.
It returns success for either skew direction within the inclusive bound, or
Invalid outside it. Unsigned absolute difference avoids signed overflow.

This is a local acceptance policy; a peer cannot choose its bound. It authorizes
no lease or payment by itself, and does not prove that a provider's time is true.
The requester result validator must apply it before committing accepted results.
