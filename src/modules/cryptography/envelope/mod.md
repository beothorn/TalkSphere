# Generic identity envelope

`header` encodes and validates the fixed 132-byte version-one header. Body parsing,
derivation and authenticated operations follow in subsequent steps. This format is
separate from Messaging's service-specific envelope and has no external AAD field.

`object` validates checked lengths and complete bounded borrowed object views.

`derivation` constructs exact context, salt and a protected content key.
