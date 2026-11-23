# Native hosted pages exercise

From the repository root on Linux with loopback networking:

```sh
cargo build --locked
cargo run --locked -- help pages
cargo run --locked --example pages -- --help
cargo run --locked --example pages -- help run
cargo run --locked --example pages -- run
cargo test --locked --example pages
```

Run creates four nodes in one operating-system process (a deliberate choice: separate reactors, identity files, stores and listeners, with every byte crossing an authenticated loopback socket, but no separate process
lifetimes, so it cannot show a host process crashing or restarting; see the limit below), each with its own protected identity file, its own listener and its own native Board, Services and Ledger journals: two hosts, a
**publisher** and a **reader**. The hosts also own native hosted placements and a participant journal and recover them before the offering is published. The publisher stands for a phone behind NAT: it only dials
out, and nothing ever connects to it. The same protected identity authenticates each connection, signs publications and (for the reader) opens the restricted page; private keys never leave Identity.

What the exercise checks, in order:

1. The publisher and the reader each connect to both hosts and fetch the host's Board and definition directly from it.
2. The publisher builds one public page snapshot (a root HTML resource, a style sheet and a text file), signs every resource publication and the manifest, and stores the same signed bytes at both hosts at the exact
   quoted prices. Its balance at each host falls by exactly the sum of those prices.
3. With the publisher offline, the reader lists the host's pages (one paid list), gets the manifest and each resource at the exact price of its known size, and assembles the page, verifying every signature, size and hash
   itself. The page is complete. The reader then repeats it asking only the second host (the first is treated as gone) and gets the identical complete page.
4. The publisher stores a newer page, encrypted to the reader's published encryption key, at one host only. The reader opens it and its secret resource; a reader holding no key learns nothing from the same publication.
   Across both hosts the reader takes the highest verified sequence as the best-known version (never claimed to be proven latest), and the host that never stored the newer page still shows the older one.
5. The publisher renews the first page's manifest placement at one host at its exact price, judges one paid byte check of a resource at the other host against a private expectation, and removes the old page there. The removal
   releases credits to the publisher exactly once, net of the removal price, and the host then lists only the newer page.

The exercise uses finite demonstration balances and removes its temporary directories after shutdown. It reuses the node composition of the peer and storage examples. Host crash and reopen behavior is covered by the module tests
(`hosted_publications::service::crash_tests`), not by this loopback profile. Offering bounds are widened in this example (4096-byte content, up to 600 seconds, 100 list results) relative to the pinned fixture. One placement is one
copy at one host: redundancy is the publisher's choice, and a host can withhold or serve an older version but cannot forge content. Default `serve` and `node-init` do not install or publish hosted publications; the client
journey plan adds that.

The publisher-served `cr2se.page` service (where the publisher itself must accept connections) is covered by the module and endpoint tests (`modules::pages`, `endpoint::tests::pages`), including the confined directory source.

Help exits 0 without opening stores or sockets. Invalid syntax exits 2 and a runtime failure exits 1. Use `RUST_LOG=talksphere::modules::hosted_publications=trace` for traces on stderr. The completion line goes to stdout.
