# Configured contact attempts

Transport.connect_contacts is valid once after Ready. Each copied contact becomes
a ConnectRequest with ExpectedPeer.Known equal to its original binary ID. Endpoint
changes never change the expected identity. Driver admission bounds simultaneous
connections before allocating handshake state. Individual refused/invalid/setup
attempts return Network in the corresponding Attempt; other attempts and the
listener continue. The result list retains expected IDs and successful local
connection handles, which do not imply authentication or remote receipt.

Connection outcomes, timeouts and peer-ID failures arrive through driver events.
No automatic reconnect, alternate endpoint lookup, contact mutation or Discovery
call occurs. A second invocation or a call outside Ready returns Runtime.
connection_count inspects retained local authority count; later event routing
removes completed connections. Contact capacity was validated before startup.
