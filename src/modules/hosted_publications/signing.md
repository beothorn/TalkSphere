# Hosted Publications signing

`signing_bytes(metadata, content)` is: ASCII `CR2SE-PUBLICATION-V1`, publication ID, publisher ID, kind code (1 byte), sequence (8 bytes
big-endian), publishedAt (8), visibility code (1), audience count (4), each audience ID in stored order, content size (8), content hash,
then the content. The signature covers ciphertext for restricted publications.

`sign_publication(signer, draft, content)` hashes the content, fills the metadata with the signer's identity and size, signs the exact
signing bytes and returns the publication with the signer's public key. `Draft` holds the publisher's choices (publication ID, kind,
sequence, claimed time, visibility, audience). Empty content, content above 16384 bytes (the finite profile's limit, as for Messaging and Storage) or a signer
that cannot sign fail with `Invalid` or `Unavailable`; the signer is Messaging's `Signer` port.

The module reproduces the three independent vectors (public resource, public page manifest, restricted feed-post envelope) byte for byte;
see `fixtures/vectors.md`, copied from the upstream proposal.
