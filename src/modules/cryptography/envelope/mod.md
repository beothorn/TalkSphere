# Generic identity envelope

`header` encodes and validates the fixed 132-byte version-one header. Complete body parsing,
derivation and authenticated operations are implemented alongside it. This format is
separate from Messaging's service-specific envelope and has no external AAD field.

`object` validates checked lengths and complete bounded borrowed object views.

`derivation` constructs exact context, salt and a protected content key.

`encrypt` validates recipients and returns complete independently protected objects.

`decrypt` verifies intended recipients and authentication before returning plaintext.

Valid generic decryption provides recipient-bound confidentiality/integrity, not
sender authentication or forward secrecy. Independent protected objects do not
establish their order or completeness. CR-10/11 tests use opaque test storage and
a local application manifest; neither is a new paid Storage implementation or
protocol chunk/manifest format. Messaging uses a separate envelope construction.
