# Protected identity key material

IdentityKeys owns independent signing and persistent encryption capabilities with
scoped erasure and no Clone, Debug or raw-byte getters. generate requests two
independent pairs from trusted entropy; failure erases any acquired staging key.
import consumes two zeroizing buffers from authorized protected storage, requires
32 bytes each, and transfers them into purpose-specific owners. This is a local
platform boundary, not a peer credential or a standardized backup format.

credential derives both public keys, exact ID and deterministic key binding;
only complete public values are returned. signing and encryption borrow their
distinct opaque capabilities for already-authorized local adapters. Ownership
of IdentityKeys is itself private authority; ordinary module results must not
include it. Callers enforce local purpose-scoped access before borrowing keys.

store transfers borrowed private inputs only through ProtectedKeySink. It calls
authorize before either private value is exposed, then store_pair exactly once;
errors propagate without retry. A sink must enforce local authorization, protect
any retained copy and erase staging on failure. Private borrows end with the call.
Storage is privileged platform functionality; the sink is not selected by a peer.
This contract does not itself claim durability or create a key store; Identity's
atomic store adapter supplies those guarantees.

Public derivation preserves the existing CR2SE public key, ID and binding bytes.
Fixed-fixture and failure tests exercise the bridge without exposing private
fields across module boundaries.
