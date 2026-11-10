# Logical assessment metadata

Input describes content before complete transfer without a wire encoding or raw
path. Complete owns a full Value; Object owns named child descriptions; Array
owns ordered children; Body declares uint64 logical length and whether final
content is UTF-8 text. A reference is neither content nor execution authority.

Bounded takes independent depth/node/byte Budget, caps depth31 and returns the
aggregate logical bytes including field names and unresolved declared lengths.
It rejects duplicate/empty names, address-space overflow and exhausted capacity
before handler dispatch/body allocation. Body_count applies the same validation
and counts unresolved references plus already owned logical byte buffers. Complete
takes Budget and returns Value only when every child already has complete content;
any unresolved reference is Unsupported, never an allocated placeholder payload.

Platform/Peer Protocol adapters retain actual reference association and require
complete content, text and family hashes before Services input validation or
execution. Providers may assess declared length/hash metadata only through an
explicit installed family implementation; the default provider refuses unresolved
content. This contract does not add a peer message or advertise a family provider.
