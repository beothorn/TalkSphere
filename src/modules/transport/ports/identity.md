# Identity capability

IdentityId is exactly 32 public bytes and grants no signing authority.
PublicIdentity contains the validated identity ID and Ed25519 public signing key.
Identity.public_identity returns an existing authorized credential, never silently
creates one. identify(public_key) applies the canonical ID derivation without
authenticating possession. sign_authentication signs exactly the supplied
Transport-created AuthenticationMessage and returns 64 bytes or DependencyError.
AuthenticationMessage.as_bytes borrows its domain-separated canonical bytes;
ordinary callers cannot construct arbitrary signing requests. No private key
crosses this port or appears in diagnostics.
