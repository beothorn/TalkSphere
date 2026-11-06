# Transport acceptance coverage

The [server integration matrix](../../../docs/transport-integration/ACCEPTANCE.md)
is the current assertion-level evidence for TR-01–TR-15 and applicable AT-C01–C04.
It combines exact codec/state tests, native primitives and protected Identity,
controlled byte segmentation/counters, actual TCP drivers and separate executable
peers. Linux direct local binding is supported; serialized local module deployment
and independent-implementation interoperability are not claimed.

Original Transport fixtures retain revision 8e848eb2e70ef7cd4490a753f9e647fdddcd99fd.
The diagnostic correction was adopted at 80f6795864ecf060e23482ee42909da4a649ecf0;
the later canonical signature profile/cleanup rules use
[d40b6278](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f).
Fixture bytes and revisions remain recorded in their test_support directories.
Upstream module contracts/acceptance/vector files are unchanged by subsequent
website-only commits as rechecked during integration.

From the root:

```sh
cargo test --locked
cargo test --locked modules::transport
cargo test --locked composition::server::real_wire
cargo test --locked --test server_peers --test server_streams --test server_runtime
```

The original composed connection suite uses strict scripted primitive boundaries;
it does not independently prove cryptography. Later real_wire scenarios use actual
verification/derivation/tag operations and an independent direct AEAD fixture oracle.
Counter exhaustion stays controlled unit state rather than huge traffic or a
production counter setter. Native release probes observe owned-secret destruction,
not compiler/register erasure or physical disk deletion.

The real TCP drain test found a local server-control admission tracking bug, fixed
in integration step 21; it was not a protocol defect. Earlier protocol reports and
verified upstream resolutions remain in CR2SE bugs. No new language-independent
protocol issue was confirmed during integration. Minimum-profile services, paid
recovery and other application modules remain outside Transport completion.
