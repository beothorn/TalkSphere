# Canonical verification keys

VerificationKey.parse receives exactly 32 public bytes. It decodes an Edwards25519
point using the established curve library, requires re-encoding to reproduce the
input and rejects points whose order divides eight. This rejects noncanonical y,
the forbidden x=0 sign bit, non-curve encodings and weak points. Wrong widths or
failed decoding return InvalidInput; noncanonical/small-order points return
AuthenticationFailed. No allocation or secret operation occurs.

A successful value exposes only canonical public bytes through to_bytes. It does
not grant trust or possession. Mixed-order points are permitted, as required by
the [CR2SE profile](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/cryptography/README.md#ed25519-verification-profile);
there is no extra prime-subgroup restriction. Signature R values use the same
point-validation rules. Signature equation alignment is the next Identity step.
