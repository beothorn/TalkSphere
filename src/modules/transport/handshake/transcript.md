# Transcript

new takes initiator HELLO followed by acceptor HELLO, independent of arrival
order. It retains their complete canonical bytes, each prefixed by a four-byte
big-endian length of 100. as_bytes borrows the resulting 208-byte transcript.

authentication(role) constructs a purpose-scoped signing message from the ASCII
AUTH domain label, a zero separator, role byte 1 or 2 and transcript. sign asks
Identity to sign that message and returns AUTH, preserving any dependency failure.
verify asks Crypto to verify the signature against the given sender role and
public key. It returns failure unchanged and supplies no replacement signature.
Neither operation alone marks a connection authenticated. Inputs are never logged.
