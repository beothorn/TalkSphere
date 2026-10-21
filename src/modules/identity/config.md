# Identity config contract

Limits.new accepts text_bytes, contacts, signing_bytes, envelope_bytes and record_bytes from trusted local policy. It rejects text below 58, envelopes below 148 and all zero capacities. Named getters return immutable independent limits without allocation or I/O. Every future operation must enforce its appropriate limit before work; these are local policy, not protocol maxima.
