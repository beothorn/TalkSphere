# Messaging fixture provenance

Source revision: [CR2SE 73edf2d](https://github.com/beothorn/CR2SE/tree/73edf2dc230bccb0f27f2ea6e06e938fff738862).

- `definition.json`: the complete section 16 JSON definition from the [specification](https://github.com/beothorn/CR2SE/blob/73edf2dc230bccb0f27f2ea6e06e938fff738862/modules/messaging/README.md#16-required-service-definition); whitespace only reformatted. The production catalogue embeds it as its independently pinned schema source.
- `offering.json`: the section 7 shape with the [acceptance harness](https://github.com/beothorn/CR2SE/blob/73edf2dc230bccb0f27f2ea6e06e938fff738862/modules/messaging/acceptanceTests/README.md) terms: all four usage factors 1/2, minimum 1, discover and status 1, remove 2, check 3, payload at most 100 bytes, retention 1 to 100 seconds, total 200, discover page 100, response 5. `PROVIDER_ID` is replaced by the provider test identity in tests.
- `generate.py` and `vectors.json`: signing bytes and signatures for MS-01 (message), MS-06 (recovery request) and MS-07 (receipt), generated with PyNaCl and Python `hashlib` and no TalkSphere code. The identities are deterministic test identities (Ed25519 seed is one byte repeated; sender 1, provider 2, recipient 3; ID is SHA-256 of `0x01 0x01` and the public key). Rust tests consume the literal bytes and also verify the Python signatures with the Rust verifier.

Regenerate with `python3 generate.py` in this directory. These are independent schema and byte fixtures, not evidence of a working Messaging provider.
