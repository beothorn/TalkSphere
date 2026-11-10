# Trusted check clocks

Clock.mark returns one local monotonic Instant plus a comparable Services Stamp,
or a typed failure. Implementations are trusted platform dependencies, not values
supplied by peers. Finite scripts allow deterministic tests without sleeps.

SystemClock.new obtains a fresh OS-random epoch and monotonic origin; randomness
failure is Unavailable with no reused fallback epoch. mark derives exact elapsed
milliseconds from one local instant with checked conversion. Restarted instances
have different epochs, so incomparable timing remains ambiguous. Provider response
deadlines start at observed input.end; requester send/receive stamps delimit the
whole challenge response interval. Wall clocks do not control these deadlines.
