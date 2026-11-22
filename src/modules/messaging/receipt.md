# Recovery request and receipt signatures

- `request_bytes` = `ASCII("CR2SE-MESSAGE-RECOVERY-REQUEST-V1") || providerId || placementId || messageId || payloadHash || recoveryId`; the recipient signs it.
- `receipt_bytes` = `ASCII("CR2SE-MESSAGE-RECOVERY-RECEIPT-V1") || providerId || placementId || messageId || senderId || recipientId || payloadHash || recoveryId || u64be(recoveredAt)`; the provider signs it.
- `sign_request(recipient, provider, placement, message, hash, recovery_id)` returns a `RecoverInput`. `verify_request(input, provider_id, recipient_id)`: the recipient ID derived from the key must equal
  the placement's recipient (otherwise `NotAvailable`, concealed like any unauthorized request) and the signature must cover this provider, placement, message, hash and recovery ID (otherwise `Invalid`).
- `sign_receipt(provider, binding, recovery_id, recovered_at)` and `verify_receipt(receipt, binding, provider_public_key)`: the sender (through a recovered status) or the recipient verifies both signatures and the two keys' identities
  against the `Binding` (provider, placement, message, sender, recipient, hash). A mismatch of any bound value, time, signature or key is `Invalid`.

The two signatures show that the recipient authorized this recovery and that the provider recorded it; they prove nothing about what anyone read. Evidence: MS-06 and MS-07 against the independent vector (identical bytes
and signatures) and rejection of every single-value mutation.
