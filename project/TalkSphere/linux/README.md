# Linux Socket Client/Server (Ports 8513 and 8512)

This is a traditional C socket project with a single executable that can run as:
- **Server** listening on TCP port **8513**
- **Client** binding to local TCP port **8512**, connecting to the server IP on port **8513**, and sending a message.

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

This produces:
- `./socket_app`

## Run

### 1) Start server

In terminal 1:

```bash
./socket_app server
```

### 2) Run client

In terminal 2:

```bash
./socket_app client <server_ip> "<message>"
```

Example (same machine):

```bash
./socket_app client 127.0.0.1 "Hello from client"
```

## Notes

- Ensure TCP ports **8513** (server) and **8512** (client local bind) are allowed by your firewall.
- If `8512` is already in use, stop the conflicting process before running client.

## Clean

```bash
make clean
```
