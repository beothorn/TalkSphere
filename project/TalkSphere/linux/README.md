# TalkSphere

This project is a p2p resorce sharing and messaging.   

It contains one program (`talksphere`) that runs as both client and server at the same time:
- **Server side**: listens on TCP port **8513** by default.
- **Client side**: binds to local TCP port **8512** by default, connects to the local server side, and sends a message.

## Project Structure

```
linux/
├── Makefile
├── README.md
├── tests/
│   └── run_tests.sh
└── src/
    ├── logging.h
    ├── main.c
    ├── program_arguments.c
    ├── program_arguments.h
    ├── socket_basics.c
    └── socket_basics.h
```

## Build

From the `linux` folder:

```bash
make
```

Output binary:
- `./talksphere`

## Run

Run with default ports:

```bash
./talksphere
```

Run with custom ports:

```bash
./talksphere <client_port> <server_port>
```

Example:

```bash
./talksphere 8999 9898
```

## Log Level

By default, TalkSphere prints `info`, `warn`, `error`, and `fatal` logs.

Use `TALKSPHERE_LOG_LEVEL` to choose the minimum log level:

```bash
TALKSPHERE_LOG_LEVEL=trace ./talksphere
TALKSPHERE_LOG_LEVEL=debug ./talksphere 8999 9898
TALKSPHERE_LOG_LEVEL=warn ./talksphere
```

Accepted values:
- `trace`
- `debug`
- `info`
- `warn`
- `error`
- `fatal`

Missing or unknown values use `info`.

## Test

Build the binary first:

```bash
make clean all
```

Run the automated happy-path and error-path checks:

```bash
make test
```

Test the default ports:

```bash
./talksphere
```

Expected output should look like:

```text
Server listening on port 8513...
Sent message to 127.0.0.1:8513 from local port 8512
Received from 127.0.0.1:8512 -> Hello from TalkSphere
```

Test custom ports:

```bash
./talksphere 8999 9898
```

Expected output should look like:

```text
Server listening on port 9898...
Sent message to 127.0.0.1:9898 from local port 8999
Received from 127.0.0.1:8999 -> Hello from TalkSphere
```

## What to Observe (Learning Notes)

- With defaults, the server prints `Received from <ip>:8512 -> ...` because the client explicitly binds to local port `8512` before connecting.
- With `./talksphere 8999 9898`, the client binds on `8999` and the server listens on `9898`.
- The code comments in `src/main.c` explain each socket API step (`socket`, `bind`, `listen`, `accept`, `connect`, `send`, `recv`) and *why* it is done.

## Troubleshooting

- If `bind: Address already in use` appears, something is already using one of the configured ports.
- Client and server ports must be different.

## Clean

```bash
make clean
```
