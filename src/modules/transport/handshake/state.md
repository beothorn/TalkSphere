# Handshake state

begin takes the local TCP role, expected contact policy and Identity/Crypto
dependencies. It obtains a public credential, fresh ephemeral key and nonce,
then returns a state and its first untagged HELLO. Callers must serialize HELLO
first and report completed writes through sent, not report queue admission.

receive accepts a complete unverified handshake frame, verifies shape/order,
derives the advertised ID through Identity and checks a configured contact pin.
A valid first HELLO produces local AUTH. Remote AUTH is checked against its role
and transcript; only after signature success does key agreement/derivation run.
Same-identity peers are accepted when their proofs succeed. Duplicate, forbidden
or malformed frames and dependency failures close the state and release secrets.

sent accepts HELLO once followed by AUTH once after receiving remote HELLO.
send_protection becomes Tagged after AUTH was completely written;
receive_protection becomes Tagged after remote AUTH and derivation succeed.
is_authenticated requires both conditions and an open state. take_established
transfers role, local/peer identity and session keys exactly once; before readiness
or after closure it returns nothing. These facts allow the owning connection
driver to emit its authenticated event, never provisional handshake data.

close invalidates the state and drops retained ephemeral/session material.
HandshakeError distinguishes Order, malformed Payload, UnexpectedIdentity,
Dependency and local Framing failures. All failures are terminal; socket closure,
safe error responses and timeouts remain the connection owner's responsibility.
After session transfer, that owner is responsible for key release.
