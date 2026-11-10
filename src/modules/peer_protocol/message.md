# Typed peer control messages

Kind enumerates board.get, service.get, board, definition, offer, propose, quote,
accept, input.end, result, result.end, failure, ledger.balance/result,
ledger.transfer/quote/accept/get/result, and ledger.claim.redeem/result.
Message.parse takes complete bytes plus node budget and retains a Control after
validating the known discriminator and its required field representations. Kind,
Value and Bytes getters return the discriminator, immutable fields and exact
original JSON. Unknown discriminators are Unsupported; malformed/null required
fields are Invalid. Unknown optional siblings remain inert.

Text fields are nonempty, modes are invoke/check, price modes fixed/metered,
uint32 fields use integer tokens and uint64 fields canonical decimal strings.
Identities must round-trip to canonical CR2SE text. Fixed identifiers/hashes,
transfer orders/public keys/signatures use canonical padded Base64 with exact
specified lengths. Transfer ID/fee fields occur together. Known pricing input
sizes and releaseCredits use uint64 strings; diagnostic message is optional UTF-8
bounded to1024 bytes. Nested Board/offering/definition/pricing objects are only
structurally admitted here; their authoritative semantic validators, permission,
role/issuer choices, fee arithmetic and legal message order remain mandatory at
the owning exchange boundary. Parsed objects alone authorize no financial effect.
