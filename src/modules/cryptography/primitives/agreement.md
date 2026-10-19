# Safe X25519 agreement

`ephemeral(key, remote)` consumes ephemeral authority; `persistent(key, remote)`
borrows persistent authority. Both require a 32-byte remote public input and return
only a protected `Shared` secret. Malformed lengths fail with `InvalidInput`.
All-zero results fail with `AuthenticationFailed` even though the backend's scalar
multiplication succeeded. X25519's specified input decoding/clamping is preserved;
this API does not invent stricter canonical-coordinate wire rules.

Dalek's contributory check uses constant-time identity comparison; it does not exit
on the first nonzero secret byte. Backend key and result objects have zeroizing
drop behavior. Consumed local capabilities release on success and failure. The
selected native multiplication is infallible for 32-byte inputs, so it has no
separate unavailable-primitive result to inject; fallible entropy/key access stay
at their declared boundaries. Raw shared results never become encryption keys.
