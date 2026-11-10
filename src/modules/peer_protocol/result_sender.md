# Committed result transmission

ResultSender.new requires receiving Invoke authority, a separate Services Observe
context, real Coordinator, original authenticated Association and finite Limits.
It obtains the retained Invocation and Completion through Coordinator.completed;
only fully acknowledged local commitment can produce a sender. Requester/provider
roles and the original key must match. Pending outcomes return Unknown and cannot
be transmitted as success or replaced by an uncharged failure.

The sender checks any logical output price/finalPrice against the final envelope,
encodes schema-bound metadata and immutable detached sources, validates complete
control size and caller grants, and queues result, bodies, result.end in order.
Accepted family/schema/factor/final-price validation remains Services-owned.
Encoding preserves already validated nonfinite semantics rather than granting a
remote document new type capabilities.

A shared private sequence also serves input transmission. It retains complete
control records by ownership, so queue bounds are checked before large encoding
copies. Failed queue admission advances nothing. queued records only local queue
completion; composition must drain all bytes before sending Transport END.
association and completion expose original routing and retained committed output.
Dropping or failing a sender never releases durable output or reverses accounting;
remote receipt and third-party payment remain separate outcomes.
