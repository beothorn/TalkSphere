# Third-party payment confirmation

Payment is Pending, Unknown, Unsettled or Settled independently of committed service
work. Receipt.authenticated requires a distinct Evidence grant, the original
issuer identity from authenticated Transport and complete TransferResult. Ordinary
Control grants cannot manufacture evidence; composition must supply a dedicated
trusted issuer exchange adapter. The receipt is owner-domain-bound and opaque.

Confirm payment requires Coordinator authority and a committed third-party operation.
It matches issuer, owner/requester, recipient/provider, transferId, exact gross/fee
and final net price, then applies the local requester debit or provider owned-credit
receipt once durably. Repeated evidence never duplicates payment. Permanent transfer
bindings prevent reuse for another invocation. Forwarded requester JSON is not a
Receipt. Payment problem records explicit issuer refusal or uncertain delivery,
never service failure or a request to execute work again. Unresolved holds remain
conservatively reserved until authenticated confirmation/reconciliation; no timeout
pretends to roll back a possible issuer transfer. No method performs hidden network I/O.
