# Header failures

HeaderError preserves invalid header length, unsupported version, nonzero flags,
unknown frame type, excessive payload length, invalid type/stream-ID combination
and size overflow. It is a local typed cause, not a wire code. Display messages
contain bounded metadata, never payloads. The connection closes on rejection;
unsupported versions never authorize a wire error response.
