# Placement

One provider's accepted agreement to retain one signed message. `Record` holds the placement ID, the message `Metadata`, the sender's public key and signature, acceptance and expiry times, the lifecycle `State`, `recoveryPrice` (the price charged for the recovery, zero until recovered), `payments` (the sum of the
store and every renewal price, the ceiling of any early-release credit), the recorded `Receipt` once recovered and the recorded `Removal` (removal ID, release credits, removal price) once removed. `Placement` adds the exact payload bytes
and can reassemble the complete `SignedMessage`; its debug form never prints the payload. A placement is never replaced.
