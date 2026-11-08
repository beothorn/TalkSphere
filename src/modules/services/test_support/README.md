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

The durable-family wrapper now serializes every acknowledged staged result,
preparation, decision and execution count into independent byte images. Reopening
constructs fresh Echo objects from those bytes. Fault stores preserve only complete
Services/Ledger records, with known-before and possible-after acknowledgement cuts.
The SV-09 matrix discards old modules/authorities and resolves all eight boundaries.
A separate Linux subprocess test uses protected real stores for all three owners,
then independently reopens the committed result. This is restart and simulated-cut
evidence; it does not claim physical power-loss or independent implementation tests.
