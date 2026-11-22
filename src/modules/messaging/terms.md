# Messaging terms

`Terms::parse(offering, publisher)` turns a provided Messaging offering into immutable exact terms: minimum price, the four rational usage factors,
fixed discover, status, remove and check prices, the maximum payload, the minimum, maximum and total retention seconds, the largest discovery page
(`1..=2^32-1`) and the check response time. Every member is required, positive (rates allow a zero numerator but need a positive denominator) and an
exact JSON integer: decimals, negatives, strings and absent members are `Invalid`. The offering must use `pricing`, never a fixed `price`, carry the
`cr2se.identity` precondition, name the publisher as its credit issuer and keep `minimum <= maximum <= total` retention. `Unsupported` identifies a
different service, version or pricing model, and `Limit` an advertised maximum whose store, recover or renew price exceeds uint64. Unknown
compatible members stay inert, and a wanted offering parses to the same terms.
