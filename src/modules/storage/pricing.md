# Exact Storage pricing

Rate contains a nonnegative uint64 numerator and a positive uint64 denominator.
Zero numerators are valid. Every calculation rejects zero denominators.

store takes positive size bytes, positive retention seconds, upload and retention
rates and a positive minimum price. It returns the larger of the minimum and the
single ceiling of the sum of exact upload and retention costs.

usage takes positive size, positive units, a rate and a positive minimum. For
retrieval units is one; for renewal it is the additional duration in seconds.
It returns the larger of minimum and the ceiling of size × units × rate.
Removal and check use their accepted advertised price directly rather than these
usage formulas. Limits and offering eligibility are checked by the caller.

Invalid denotes zero required inputs/denominators; Limit denotes a final price
that cannot fit uint64. There are no floating point conversions, mutations or
network effects. All output prices are exact integer credits.

## Overflow argument

A rate denominator fits uint64. If the three-factor usage numerator exceeds
uint128, its quotient by any valid denominator necessarily exceeds uint64, so
rejecting that multiplication overflow cannot reject a representable result.
A zero numerator yields zero without a spurious intermediate overflow because
the product of size and units always fits uint128.

For store, divide each term into whole and remainder. Cross-multiply only the
remainders by the opposite denominators: each product fits uint128. Compare
their sum against the denominator product using subtraction, without forming an
overflowing sum. The combined fractional ceiling is zero, one or two. Add this
to both whole parts with checked arithmetic and convert only the final result.
