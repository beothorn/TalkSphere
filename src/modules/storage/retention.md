# Retention clocks and terminal states

Anchor contains logical Unix-second progress and its last coherent Time
observation. new anchors an initial UTC estimate. advance takes a new observation
and optional trusted reboot elapsed time, returning a new immutable anchor.

Within one boot, only boot-local elapsed seconds consume retention; a wall-clock
adjustment cannot shorten a lease. Backward elapsed time or unexpected reboot
evidence is Invalid. Across boots, composition must supply independently trusted
elapsed seconds since the saved observation, including offline time. Without it,
advance returns Unavailable; wall-clock subtraction alone is not that evidence.
Logical time overflow is Limit. The authoritative owner must durably persist the
anchor and lease transitions together before relying on them after restart.
No concrete persistent time source is supplied by this helper.

expiration takes acceptance Unix seconds and positive retention seconds. It
returns checked expiration, Invalid for zero retention or Limit on overflow.
refresh takes a candidate mutable Lease and logical Unix seconds. It changes
Active to Expired when time reaches expiry, returning true only for a transition.
Receiving, Removed and Expired remain unchanged even after a clock rollback.
The caller must authorize lookup first and durably commit the candidate before
serving affected requests. Physical byte cleanup and accepted read pins belong
to the owner; this helper never deletes bytes or changes accounting.
