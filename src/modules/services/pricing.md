# Exact service pricing

Price.Fixed is an exact positive credit amount. maximum validates positivity;
final_price requires no post-execution billed duration and returns the same amount.
quote obtains pricing from the pinned installed requester contract using bounded
provider factors and immutable offering terms, then checks the explicit requester
maximum. Invalid, Unsupported and Insufficient fail before execution/settlement.
No arithmetic uses floating point or adds undisclosed fees.

Requester.pricing defaults to family-validated positive fixed prices. Calculated
family models require an installed override; a remote pricing.model string cannot
supply executable pricing rules. Agreed third-party fees and family adjustments
are separate immutable acceptance data, never inferred from a successful result.

Price.Computation stores positive minimum, nonnegative rate numerator, positive
rate denominator and positive maximum billed milliseconds. Only the exact
cr2se.computation version1 contract may use it. maximum and final_price use u128
products and quotient/remainder ceiling division; final billed time must be in
1..accepted maximum. Zero numerator is valid and charges the minimum. Conversion
back to uint64 is checked. Literal rate1/1000, max60000 gives maximum60; billed1201
gives2. Runtime completion shape/outcome validation remains with the installed
Computation contract; language errors and time limits can be billable completions.
