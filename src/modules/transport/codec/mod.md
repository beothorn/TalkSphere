# Transport codec exports

This module exposes FrameHeader, FrameProtection, FrameType and HeaderError for
preliminary version 1 header validation, plus the [control payload module](control/README.md).
The [handshake payload module](handshake.md) exposes Hello, Auth and their typed
shape errors; cryptographic verification remains outside these codecs.
The [codec guide](README.md) describes header operations and validation boundaries.

Public constants are VERSION = 1, HEADER_BYTES = 12, INTEGRITY_TAG_BYTES = 16
and MAX_PAYLOAD_BYTES = 65,536. They are fixed wire values, not configurable
peer limits. There are no module-level functions or mutable public fields.

The future Transport reader and writer compose these formats; application
protocols consume authenticated streams rather than invoking control codecs.
Parsing a header or payload does not establish authentication or stream state.
