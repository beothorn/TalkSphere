# FrameReader

Default creates an empty reader for one socket. poll_frame takes a socket,
polling context and direction-specific integrity phase; it returns Pending,
one complete unverified WireFrame, boundary EOF (None), or FramingError.
The caller retains the same reader and socket across polls and changes phase
only between completed frames. No input beyond the current frame is consumed.

Malformed headers fail before body allocation, partial EOF is Truncated, and
socket errors latch permanently. Re-polling a failed reader returns Closed;
re-polling after clean EOF returns None. The reader never closes the socket
itself: the connection owner must terminate after a failure.
