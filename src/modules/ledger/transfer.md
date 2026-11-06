# Canonical transfer orders

Order contains transferId16, issuer32, owner32, newOwner32 and positive uint64
amount. Encode emits exactly 121 unsigned bytes: version1 at0, ID at1, issuer17,
owner49, newOwner81, big-endian amount113. Public key and signature are external.
Decode accepts only this exact layout and rejects old105-byte orders, unsupported
versions, missing/trailing bytes, zero amounts and recipient equal to owner/issuer.
This codec performs no signing, authentication, fee acceptance or accounting.
