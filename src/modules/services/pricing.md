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
