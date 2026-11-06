# Accepted settlement terms

Terms retains requester/provider/issuer identities, positive maximum, fixed versus
metered price, exact accepted contract/quote bytes, optional transfer identifier
and exact fee, and the total spending bound. These are copied from authorized
Services acceptance; descriptions or peer payload fields cannot supply authority.

Assess requires the local identity to participate and validates arithmetic, payment
field presence and spending bounds. Same requester/provider uses Internal mode;
otherwise issuer=provider selects Provider, issuer=requester selects Requester,
and a third identity selects ThirdParty. Only the provider enforces its local
nonzero requester trust and explicit requester-credit/third-party risk acceptance.
A requester never asserts trust on the provider's behalf. Policy is trusted local
composition data and defaults to refusing optional credit modes.

Third-party mode requires transferId16 and fee, checked maximum+fee within total
spending bound. Other modes forbid transfer fields. Terms remain immutable in the
operation binding even if later trust, offerings or fee policies change. This
format is local and introduces no peer wire fields or zero-price service holds.
