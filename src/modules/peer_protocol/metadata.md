# Uncharged metadata responses

Respond takes a real Board Provider, an owner-issued Board Read Context and a
validated board.get or service.get Message. It pins one current complete snapshot
and returns a single Control Record embedding the exact original Board or bound
definition document. The Board response contract checks complete wrapper size and
depth before allocation; no alternate binary64 parser or truncated response is
introduced. A missing publication/offering returns failure with not_available,
never an invented definition. Other Board errors remain typed, including denied
local authority, cancellation, capacity, storage failure and uncertainty.

This operation performs no Services dispatch, Ledger mutation or socket I/O.
Trusted composition must additionally enforce Peer Protocol authority, authenticated
stream association, control-rate quotas, response ordering and END in both
directions through the exchange/Transport adapters. It is not a standalone free
application service or a bypass for publication capability validation.
