# Handshake payload contracts

Hello exposes three 32-byte public fields: identity_public_key (Ed25519),
ephemeral_public_key (X25519), and nonce. encode returns exactly 100 bytes with
selectors fixed to one; decode requires exactly 100 bytes, rejects any selector
at offsets 0, 1, 2 or 35 other than one, and copies the fields. No key validity,
entropy, peer identity or ownership is established by decoding.

Auth exposes a 64-byte signature. decode checks this exact width and encode
returns the unchanged bytes. Zero signatures are not specially rejected here;
signature verification belongs to the cryptographic adapter and handshake driver.

HandshakePayloadError reports Length with expected/actual byte counts or
Selector with its byte offset and received value. No received bytes are logged.
Both payload values are copyable; the connection driver controls their lifecycle.

Reference: [Transport sections 5.1–5.2, revision 8e848eb](https://github.com/beothorn/CR2SE/blob/8e848eb2e70ef7cd4490a753f9e647fdddcd99fd/modules/transport/README.md#51-hello-payload).
Run `cargo test --locked modules::transport::codec::handshake` from the root;
build/run instructions and scope are in the [codec guide](README.md).
