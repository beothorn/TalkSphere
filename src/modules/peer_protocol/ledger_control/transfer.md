# Issuer transfer negotiation

Config supplies the original real Ledger and its authority, receiving Peer Protocol
authority, finite Limits and trusted Policy. Fixed is an explicit nonnegative local
fee policy, including zero. Time supplies trusted local monotonic milliseconds and
finite expiry; no peer timestamp controls lifetime. Expiry is clipped to local
Peer Protocol/Control deadlines. Issuer owns only finite transient fee quotes.

begin validates the authenticated owner/issuer association and exact121-byte order,
32-byte public key and64-byte signature through Ledger's strict public ports.
An existing signed accepted retry returns Begin.Stored before consulting new fees,
maximumFee or current capacity. A new order calls local Policy and checks fee below
gross and within the explicit maximum, then returns Begin.Quote. Insufficient at
this quoting stage means fee refusal and maps to price_rejected. Quote creation
reserves no funds. Quoted exposes original association and exact fee control only.

accept requires the original owner instance, stream, ID, exact fee and live expiry.
Ledger consumes the same-stream fee capability, rechecks current balance/headroom
under its writer and commits the original transfer once. Response preserves original
gross/fee/net. Wrong acceptance or current insufficient funds produces no new
transfer. Durable uncertainty remains Unknown. Failed sends retain stored results;
discard removes only unaccepted transient quotes. No administrative grant, credit
creation, service execution or ordinary work retry is exposed.

The fixed public reply shape is preflighted under local control-node limits before
monetary commitment. A later unexpected encoding failure retains Unknown rather
than claiming a definitive no-effect failure. Interrupted transmission still
requires the original durable owner's replay/status rules.
