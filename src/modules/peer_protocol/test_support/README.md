# Deterministic Peer Protocol dependencies

Test-only ordered DATA and terminal event scripts distinguish normal END,
cancellation and lost replies. Finite outbound capacity refuses without copying
bytes; explicit capacity changes model writable notifications. Generic counted
reply scripts represent Board, Services, Ledger or entropy ports and fail closed
on exhaustion. No sleep, socket authentication, production registration or
financial authority is synthesized. Future typed adapters wrap these scripts at
their public contract boundary. Exercise from the root with
`cargo test --locked modules::peer_protocol::test_support`.
