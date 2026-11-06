# Finite claim commitments

Create claim requires Administer, a durable local creation attempt key32, positive
amount, optional authorized redeemer restriction and secure entropy. Independently
generated ID32/secret32 are guarded from acquisition; only SHA256(secret), authoritative
amount, restriction, creation key and state are persisted. AVAILABLE reserves finite
issuance; grants and service holds cannot consume it. Capacities reports unreserved
issuance/internal budgets under Read authority. Zero/self restriction, collisions,
repeated creation keys, full replay capacity, entropy loss and storage failures reject.

A successful call returns guarded Portable bearer material. If acknowledgement or
private export is lost, secret material cannot be reconstructed from the commitment;
recover then query Claim creation by the original attempt key to obtain public ID/state
and explicitly invalidate it if appropriate. Never retry as a new issuance blindly.
Terminal records and creation keys are retained for Ledger lifetime. State is Available,
Redeemed (one recorded owner), or Invalidated. Claim use never raises local trust.
