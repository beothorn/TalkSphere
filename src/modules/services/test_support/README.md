# Services deterministic dependencies

This cfg(test)-only module supplies a finite atomic-register failure script,
explicit monotonic clock and generic dependency reply/call-count script.
Before-write failures preserve old durable bytes; after-write reply loss returns
Unknown while retaining new bytes. Reopening copies only durable bytes.
Script exhaustion fails closed. Literal response7 and binary00ff do not depend
on production pricing or codecs. No sleeps, sockets, production registration or
peer authentication are involved.

Typed echo requester/provider fixtures exercise assessment, execute, completion,
prepare, commit, abort and status with an explicit execution counter. Real Board
publication/selection fixtures use only public contracts and the installed Services
registry. The echo state is deliberately volatile; later durable-coordinator tests
must supply a persistent family participant rather than claiming this fixture
survives process restart.

From the root: cargo test --locked modules::services::test_support.
