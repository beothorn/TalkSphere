# Linux Socket Client/Server (Ports 8513 and 8512)

This project is a **learning-focused** C socket example.

It contains one program (`socket_app`) that can run in two modes:
- **Server mode**: listens on TCP port **8513**.
- **Client mode**: binds to local TCP port **8512**, connects to server port **8513**, and sends a message.

## Project Structure

```
linux/
├── Makefile
├── README.md
└── src/
    └── main.c
```

## Build

From the `linux` folder:

```bash
make
```

Output binary:
- `./socket_app`

## Run

### 1) Start server (terminal 1)

```bash
./socket_app server
```

### 2) Start client (terminal 2)

```bash
./socket_app client <server_ip> "<message>"
```

Example on same machine:

```bash
./socket_app client 127.0.0.1 "Hello from client"
```

## What to Observe (Learning Notes)

- The server prints `Received from <ip>:8512 -> ...` because the client explicitly binds to local port `8512` before connecting.
- The server listens on `8513`, so client destination is always `<server_ip>:8513`.
- The code comments in `src/main.c` explain each socket API step (`socket`, `bind`, `listen`, `accept`, `connect`, `send`, `recv`) and *why* it is done.

## Troubleshooting

- If `bind: Address already in use` appears for client, something is already using local port `8512`.
- If `connect` fails, ensure server is running and firewalls allow TCP `8513`.
- If testing across machines, use the server machine's reachable IP (not `127.0.0.1`).

## Clean

```bash
make clean
```
