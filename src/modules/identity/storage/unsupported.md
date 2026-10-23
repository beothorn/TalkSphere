# Unsupported protected storage platforms

FileStore.open accepts the same trusted path and Limits as the supported adapter,
but always returns Unsupported and creates no filesystem state. No platform may
silently fall back to unprotected private-key storage. The Linux implementation
is the only currently tested production platform.
