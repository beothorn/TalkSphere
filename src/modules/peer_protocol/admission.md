# Read-only economic preflight

Roles records exact requester, provider and issuer from a validated selection.
Gate.check validates local participation, provider trust and explicitly accepted
requester/third-party issuer policy. Gate.capacity takes exact positive maximum
and fee and checks current finite resources without reserving or mutating credit.

LedgerGate.new takes the expected local identity, actual Ledger, a separately
issued Read grant and immutable Policy. A mismatched owner is Denied. Provider
trust must be positive for external work; requester-issued or third-party risk
requires its configured opt-in. Capacity reads available issued/owned credits,
finite issuance/internal budgets or incoming uint64 headroom according to actual
roles. Arithmetic and insufficient resources stay typed. Incoming preflight cannot
guarantee capacity against concurrent holds; Services/Ledger acceptance must still
reserve atomically under the original key. No method signs, transfers, grants,
changes trust or creates a financial reservation.
