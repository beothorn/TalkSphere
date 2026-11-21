# Native Storage adapters

`Platform` supplies raw readings: the boot identifier text, kernel uptime text and UTC
seconds. `Linux` reads the real values. `NativeClock` turns them into a Storage `Time`
(UTC seconds, uptime seconds and a 16-byte boot tag hashed from the boot identifier).
Within one boot, uptime stays comparable across process restarts; a different boot
tag means there is no trusted elapsed time and Storage refuses rather than guess.
Unreadable or malformed readings return an explicit unavailable or invalid error.

`SystemCrypto` supplies 32-byte unpredictable lease identifiers from operating-system
entropy and exact SHA-256 hashes; failure is reported, never papered over.
