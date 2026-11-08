# Local serialized coordinator adapter

This version-one binding is an in-process/local contract exercise, not CR2SE peer
JSON, authenticated Transport, remote admission or independent-process interoperability.
Trusted composition supplies the actual coordinator and an out-of-band Context;
encoded fields cannot create grants, install code or impersonate a different owner.

Adapter takes a coordinator reference and positive finite message Limits. Encode
accepts a Request and returns an owned complete bounded buffer. Dispatch validates
the version, exact envelope and key then calls the same coordinator methods as a
direct caller. Requests support input, start, stage, prepare, decision, commit,
abort, status and result for already accepted work. Quote admission remains typed
and separately provenance-checked; this adapter cannot fabricate Accepted handles.
The six envelope fields are version:uint32, action:uint32, requester:bytes32,
connection:bytes32, stream:positive uint32 and typed payload. Input carries a Value,
decision a boolean; other actions require a false sentinel. Actions are numbered
0–8 in the order above. The shared codec preserves widths, IEEE bits and raw bytes.

Mutation replies are true. Status returns absent or the exact Phase variant name.
Result returns [logical output, original result bytes, uint64 final price], never
private handler tokens. Decode validates a complete reply and returns owned Values.
No streaming queue or asynchronous retained client buffers exist; slow consumers
retain only their own finite reply. Results remain in the coordinator after their
session/buffer disappears. Lost replies require original-key status/recovery;
start is never retried automatically. Reply encoding failure after a mutation is
Unknown, since the effect may already have occurred. All underlying authority,
version, cancellation, state, storage and uncertainty errors remain explicit.
