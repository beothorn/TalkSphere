# Hosted Publications terms

`Terms::parse(offering, publisher)` turns a provided `cr2se.hosted-publication` version-one offering using pricing model
`cr2se.hosted-publication.v1` into immutable exact terms: the minimum price, the four rational factors (`contentByteUsageFactor`,
`contentByteStorageDurationFactor`, `readByteUsageFactor`, `renewalByteStorageDurationFactor`), the fixed list, remove and check prices,
`maximumContentBytes`, the minimum, maximum and total retention seconds, `maximumListResults` and `maximumManifestResources`
(`1..=2^32-1`), `maximumAudience` (`0..=65535`, zero meaning public only) and `checkResponseSeconds`.

Every member is required and an exact JSON integer; counts and prices are positive (factors allow a zero numerator but need a positive
denominator). The offering must use `pricing`, never a fixed `price`, carry the `cr2se.identity` precondition, name the publisher as its
credit issuer, keep `minimum <= maximum <= total` retention and advertise at most 16384 content bytes (the finite profile's limit; the
upstream example's 10000000 does not fit). `Unsupported` identifies another service, version or pricing model; `Limit` an advertised maximum
whose store, get or renew price exceeds uint64. Unknown compatible members stay inert.

Prices (credits, one upward rounding, floored at the minimum, arbitrary-precision intermediates):

```text
storePrice(S, T) = max(M, ceil(S*B.n/B.d + S*T*X.n/X.d))
getPrice(S)      = max(M, ceil(S*R.n/R.d))
renewPrice(S, A) = max(M, ceil(S*A*N.n/N.d))
```

Zero sizes or durations are `Invalid`; an exact result above uint64 is `Limit`. The definition served by `service.get` is added with the
Services adapter (plan step 12).
