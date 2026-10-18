# Injected dependencies

`Dependencies` bundles socket provider, monotonic clock, one authorized local
Identity and Cryptography adapter. Composition owns these values; peers never
supply them. Dependency identifies Socket, Clock, Identity or Cryptography as a
failure boundary. DependencyError distinguishes unavailable capability, denied
operation, invalid input and failed verification without secret diagnostics.
