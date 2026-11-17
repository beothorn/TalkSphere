# Storage fixture provenance

Source revision: [CR2SE d40b6278](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f).

- definition.json: complete section 13 JSON definition from the [specification](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/README.md); whitespace only reformatted.
- offering.json: section 3 shape with [acceptance harness](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/storage/acceptanceTests/README.md) terms: all rates 1/2, minimum1, remove2, check3, size/range100, retention1..100, total200, response5. Placeholder issuer replaced with the public Transport vector initiator ID, solely as a test identity.
- prices.json: literal ST-01/05 expectations. Store rows are size/seconds/price; retrieve rows length/price; renew rows size/additionalSeconds/price. No production pricing helper generates expectations.

The `abc` hash is literal ST-02 evidence in fixture_tests.rs. These are independent
schema/hash fixtures, not evidence of a working Storage provider or node.

The production standard catalogue embeds definition.json as its independently
pinned schema source; offering.json and price/hash scenarios remain test evidence.
Remote input never replaces the embedded catalogue. Recognition alone does not
install requester/provider code or advertise a usable paid service.
