# TalkSphere Docs

## Linux prototype behavior (current)

Each process runs a server and waits for inbound connections.

Command:
```bash
./talksphere <instance_port> <reserved_port>
```

Message support:
- `CONNECT:<target_host>:<target_port>,FROM:<reply_host>:<reply_port>`
- `MESSAGE:<text>`

The `CONNECT` message currently triggers:
1. outbound `MESSAGE:Hello` to the target
2. outbound `MESSAGE:Hello` to the `FROM` endpoint

See `project/TalkSphere/linux/README.md` for the full nc walkthrough.
