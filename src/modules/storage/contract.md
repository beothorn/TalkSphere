# Lease descriptions and platform dependencies

LeaseId and ContentHash are exactly 32 bytes. A lease ID is provider-generated,
unpredictable and unique within that identity, and grants no authority. Hashes
cover exact logical bytes. State is receiving, active, expired or removed.

Lease holds ID, authenticated requester/provider identities, immutable original
accepted terms, byte size, whole-object hash, durable acceptance/expiration Unix
seconds, original credit price and logical state. It exposes no filesystem path.
This description alone neither creates a lease nor authorizes a mutation.

Time contains a UTC Unix-second estimate, boot-local elapsed seconds and a
16-byte opaque boot identifier. Compare elapsed times only within one boot.
Clock.now returns a coherent observation or a typed Services error; composition
must explain UTC correction and reboot assumptions. Crypto.lease_id returns one
secure random candidate or fails; uniqueness remains the owner's responsibility.
Crypto.hash accepts exact bytes and returns SHA-256 or a typed failure. Neither
port may fabricate successful output after dependency failure.

Limits contains aggregate bytes, lease slots, pending operations, retained result
bytes and per-owner operations. validate returns the unchanged policy when all
limits are positive and per-owner operations fit aggregate operations, otherwise
Invalid. It performs no allocation, effects or publication. Quota reservation,
authorized lease lookup and durable participant storage are later capabilities.

Persistence.load takes a finite maximum before allocating and returns the latest
snapshot or absence for a fresh store. Corruption, unsupported versions, limits
and storage failure are explicit errors. replace atomically persists exact bytes
with the adapter's required durability barriers. Possible commitment followed by
acknowledgement loss returns Unknown. A single authoritative owner serializes
writes; this port does not itself supply cross-process writer ownership.
