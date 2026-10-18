# Frame type registry

FrameType preserves version-1 values: HELLO 1, AUTH 2, PING 3, PONG 4, GOAWAY 5,
ERROR 6, OPEN 16, DATA 17, END 18 and CANCEL 19. Byte conversion rejects every
unassigned value. HELLO/AUTH/PING/PONG/GOAWAY use stream zero; OPEN/DATA/END/CANCEL
require nonzero streams; ERROR can have either scope. Application protocols use
OPEN identifiers rather than allocating new frame types.
