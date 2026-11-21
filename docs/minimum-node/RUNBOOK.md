# Minimum useful node: operator guide

A node is one directory (the *node root*) holding its identity, durable owners, lease catalog and
control socket. It runs on Linux and is controlled locally. Build with `cargo build --locked`.

## Create and run

```sh
talksphere node-init --node-root ~/ts-a            # prints the public CR2SE ID
talksphere node-show --node-root ~/ts-a            # inspect without starting
talksphere serve --node-root ~/ts-a --listen 127.0.0.1:7001
```

`node-init` takes an optional `--config FILE` (`node.json`): `{"version":1,"offering":null}` makes a
requester-only node; `offering`, `capacity`, `credits` and `control` members override finite defaults
(see [the configuration contract](../../src/composition/node/config.md)). The node reports
`ready <ID> <ADDRESS>` and stops on SIGINT or SIGTERM after draining accepted work (`--shutdown-ms`).
Changing `node.json` takes effect at the next start; a withdrawn offering discards only the derived Board
snapshot, and the `storage/` leases are not served until an offering is configured again.

## Credits and trust

Two operators agree finite credits out of band, then each records its side (provider B, requester A):

```sh
talksphere node-ctl --node-root ~/ts-b grant --peer <A-ID> --amount 100000   # B issues
talksphere node-ctl --node-root ~/ts-b trust --peer <A-ID> --millionths 1
talksphere node-ctl --node-root ~/ts-a own   --peer <B-ID> --amount 100000   # A records what it holds
talksphere node-ctl --node-root ~/ts-a trust --peer <B-ID> --millionths 1
```

`grant` is applied once per `--request-id` while the node runs; `own` and `trust` are absolute.

## Paid Storage

```sh
CONTACT='<B-ID>?address=127.0.0.1&port=7001'
talksphere node-ctl --node-root ~/ts-a fetch    --contact "$CONTACT"
talksphere node-ctl --node-root ~/ts-a store    --contact "$CONTACT" --file ./object --retention-seconds 60 --max-spend 100
talksphere node-ctl --node-root ~/ts-a retrieve --lease <LEASE> --max-spend 10 --output ./copy
talksphere node-ctl --node-root ~/ts-a check    --lease <LEASE> --max-spend 10 --expected-file ./object
talksphere node-ctl --node-root ~/ts-a renew    --lease <LEASE> --seconds 30 --max-spend 100
talksphere node-ctl --node-root ~/ts-a remove   --lease <LEASE> --max-spend 10
talksphere node-ctl --node-root ~/ts-a status
```

Spending always needs `--max-spend`; the exact quoted price is charged. Objects are limited to 16384 bytes.

## When something goes wrong

- `unknown_outcome`: the remote effect may have committed and was not retried. List these with
  `node-ctl outcomes --unknown`, check the provider's accounting with `balance`, and do not assume a
  store failed. A new `store` is a separate lease and a separate charge.
- `unavailable`: the peer or a local dependency could not be reached; nothing was charged.
- A crash needs no repair: restart with the same command; owners recover before the node listens. If
  startup fails, the message names the owner; nothing is repaired silently.
- Provider restart across a machine reboot is refused: retention needs trusted same-boot uptime.
