# Explicit transfer owner consent

Signed contains an exact positive gross Order, public key/signature and nonnegative
maximum fee; signing/private keys remain outside this adapter. Request.new requires
local Request authority, original authenticated issuer association and strict
canonical owner proof. bytes borrows the bounded original transfer control.

quote requires separate incoming Invoke permission, original stream, correct
transfer ID, fee below gross and within the requested maximum. It produces an opaque
Offer with read-only fee/id, without accepting it. accept is a separate explicit
local Request action, checks original owner/domain/fee and returns the exact
ledger.transfer.accept control. Receiving a quote alone never authorizes commitment.

finish requires the same issuer stream and normal completion. A newly quoted
transfer needs exact local fee acceptance before its result; order and final fee
must remain original. Completion retains opaque original-issuer Transfer proof and
accepted_fee. Direct stored retries have accepted_fee=None: their historical fee
may exceed a newly sent maximum because no new fee is being agreed or charged.
This observation does not imply fresh acceptance or automatic local balance changes.
Separate settlement still compares original agreed order/fee/payment exactly.

Abnormal completion stays Unknown. Transfer request interruption is conservative
even before a new quote: the same signed order may already have a durable stored
result. No lost reply is proof of rollback and no automatic generic work retry is
introduced. Explicit repeated signed transfer orders use Ledger's lifetime guard.
