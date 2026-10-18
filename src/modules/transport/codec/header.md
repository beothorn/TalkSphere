# Frame header contract

FrameHeader.new(type, stream_id, payload_length) validates the version-1 payload
bound and static type/ID combinations. Private fields preserve that validation.
encode emits 12 bytes: version 1, registered type, zero flags, big-endian stream
ID and payload length. decode requires exactly 12 bytes and rejects unsupported
version, nonzero flags, unknown type, excessive length or invalid ID combination.
Accessors return type, ID and payload length without implying authentication.

FrameProtection selects untagged or tagged overhead. frame_len uses checked
arithmetic for header plus payload and, when tagged, 16 bytes. Payload length
excludes the tag. Maximum payload is 65,536 bytes. Header rejection never scans
for a replacement boundary; the connection driver applies safe closure policy.
