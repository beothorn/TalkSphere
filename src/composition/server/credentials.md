# Protected server identity startup

load takes validated configuration and an explicit stored-contact selection.
It requires an existing protected Linux identity directory, retains its exclusive
writer lock, validates the complete key association, and grants only ReadPublic
and Sign authority to TransportIdentity. Credentials contains that adapter, the
public local ID and copied endpoint-bearing contacts. It grants no backup/import
or arbitrary peer signing interface. No network dependency is constructed here.

Stored contacts replace explicit contacts, never merge; selecting both fails.
Stored snapshots are validated against configured contact capacity and endpoint
rules before use. Missing/corrupt snapshots fail explicitly when selected.
Missing/insecure/corrupt keys or denied grants return the secret-free Identity
failure category without replacing keys. Dropping Credentials releases private
capabilities and the protected writer lock. Non-Linux runtime composition must
return Unsupported rather than provide an unprotected fallback.
