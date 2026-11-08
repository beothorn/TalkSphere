# Services deterministic dependencies

This cfg(test)-only module supplies a finite atomic-register failure script,
explicit monotonic clock and generic dependency reply/call-count script.
Before-write failures preserve old durable bytes; after-write reply loss returns
Unknown while retaining new bytes. Reopening copies only durable bytes.
Script exhaustion fails closed. Literal response7 and binary00ff do not depend
on production pricing or codecs. No sleeps, sockets, production registration or
peer authentication are involved.

Board-selection fixtures and typed handler lifecycle adapters will be added when
the public handler and registry contracts exist. Consequently plan step3 remains
in progress until those adapters are verified through their owning contracts.

From the root: cargo test --locked modules::services::test_support.
