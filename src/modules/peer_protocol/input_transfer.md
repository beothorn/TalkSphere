# Complete input gates

InputTransfer consumes an opaque Incoming reservation under the receiving owner's
Invoke authority. It keeps the original Association, local deadline and a bounded
provisional Receiver. body accepts only ordered declared bytes before the marker.
marker requires exactly input.end after all bodies are complete; it does not
substitute for Transport END. end consumes the gate only after both requirements
and returns CompleteInput. Wrong association/authority does not mutate another
stream; malformed body/order poisons the provisional receiver. Cancellation,
missing/duplicate markers, missing bytes and invalid UTF-8 never yield input.

CompleteInput exposes the assembled logical value after terminal completion.
dispatch supplies it to the real Coordinator for independent schema, length/hash
and family validation, then journals one execution attempt before calling the
provider. Errors retain original-key recovery obligations; complete network input
alone is not successful service output or authorization to charge.

InputSender consumes an Outgoing requester reservation and retains immutable body
sources and the original invocation. enqueue_next queues accept first, each body
in descriptor order next, then input.end. A full bounded queue leaves that stage
unchanged. queued means only local queue admission: composition must drain every
queued byte before sending requester END. The original complete requester input
was already independently validated by Services; encoding preserves that contract.
Drop/cancellation does not erase its durable accepted operation or financial hold.
