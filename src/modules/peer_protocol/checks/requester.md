# Requester check consent

Owner.request takes outgoing Request and Services Request grants, original
Association, retrieved Selection, complete defined challenge and explicit Spending.
It validates installed remote Rules, challenge schema/shape, mode, price, response
budget, issuer/payment and total bound. Only bounded complete challenge metadata
is supported for pre-quote resource authorization. Request holds exact propose
JSON and the locally owned challenge, never a private expected byte.

Owner.validate consumes its own Request only on the original authenticated stream
under separate incoming Invoke authority. The complete quoted offering must match
the retrieved contract. Issuer/payment must remain original, priceMode is fixed,
pricingInputs is empty and releaseCredits is absent. Installed requester Rules
independently calculate checkPrice or included-zero terms and require exact price
and total ceiling agreement. Candidate exposes exact received bytes, hash and price
without spending. Supported economic changes still need explicit consent.

Owner.consent requires a locally approved exact hash and total ceiling under both
original and current policy. Owner.reserve consumes that opaque Consented token,
uses separate quote/Coordinate grants and authoritative Current validation, and
asks Services to admit the frozen original agreement. It supplies the defined
challenge and journals one outbound check before Outgoing exposes accept JSON.
Duplicate local attempts cannot dispatch or reserve twice. Failed sends retain
original-key durable obligations; no implicit retry or refund is performed.
