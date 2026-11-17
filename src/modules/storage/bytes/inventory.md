# Protected byte inventory

Store.inventory takes a finite maximum entry count and walks the retained
directory descriptor. It returns Entry values with opaque ID, actual local byte
length and a pending flag. Fixed lock/metadata names are excluded. Canonical
lowercase object/upload identifiers, regular-file ownership, modes, link counts
and no-follow opens are checked before publication. Unknown names fail closed.

Limit rejects excess entries before collection growth; Invalid rejects malformed
names; Storage/Denied preserve unavailable or unsafe entries. The inventory
neither deletes bytes nor treats a filename as requester authority. Recovery
must account for every object and explicitly resolve provisional or unreferenced
entries before admitting new work; absence from a lease snapshot alone does not
infer a coordinator abort.
