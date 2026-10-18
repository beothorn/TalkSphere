# Integrity contracts

Integrity.new consumes SessionKeys and begins send/receive sequences at zero.
seal accepts an untagged WireFrame and Crypto, requests the exact tag over header
and payload, and returns a tagged frame while reserving the send sequence.
Busy means the preceding sealed frame has not completed; other failures release
both keys. sent commits the pending sequence after complete output only.

verify takes a complete tagged frame and Crypto. It uses the expected receive
nonce, verifies all header/payload bytes, rejects exhaustion without dispatch,
and only then advances and returns VerifiedFrame. Failure releases keys and
must close the socket without an integrity-error reply. close explicitly
releases material and is_closed reports that state. No counter can wrap.

VerifiedFrame exposes header, borrowed payload and ownership transfer of payload.
These bytes passed the injected verifier, but semantic/state validation remains
necessary. No public constructor bypasses verification.

IntegrityError distinguishes Closed, Busy, Shape, Exhausted, Dependency and
Framing. An application error response is never an automatic consequence of
these local results. Retrying failed authentication on the same session is forbidden.
