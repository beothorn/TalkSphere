# Stream domain

`ids` provides per-connection allocation and history. Connection orchestration
owns mutation; codecs parse bytes and integrity verifies them before stream
state is consulted. Application consumers do not choose wire IDs.
