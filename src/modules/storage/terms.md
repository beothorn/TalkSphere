# Validated Storage offering terms

Terms.parse consumes a common-validated immutable Board offering. It requires
cr2se.storage version 1, calculated model cr2se.storage.v1, identity precondition
and all required economic and info members. A fixed price is forbidden.

The returned Terms contains minimumPrice, four exact rates, removePrice,
checkPrice, maximumBytes, minimum/maximumRetentionSeconds,
maximumTotalRetentionSeconds, maximumRetrieveBytes and checkResponseSeconds.
Rates permit zero numerator and require positive denominator. Every other numeric
term is positive uint64; retention minimum <= maximum <= total maximum.
Unknown compatible additions remain inert.

Before returning, calculate store, retrieve and renewal prices at advertised
maxima. An offering that could admit an unrepresentable price is rejected even
if the immediately requested operation would cost less. Terms does not authorize
capacity, spend, execute work or alter existing leases. Later operations validate
current offered terms while original accepted obligations remain unchanged.

Unsupported identifies a different family/version/model. Invalid identifies
malformed required fields, fixed-price use, missing identity precondition or
inconsistent limits. Limit identifies exact final-price overflow. Inputs must
already satisfy common Board parsing and finite document limits.
