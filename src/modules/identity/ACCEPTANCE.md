# Identity acceptance evidence

Baseline: [CR2SE d40b6278](https://github.com/beothorn/CR2SE/tree/d40b6278f24430e814d7fa82094f57261bfcac9f).
The [Identity cases](https://github.com/beothorn/CR2SE/blob/d40b6278f24430e814d7fa82094f57261bfcac9f/modules/identity/acceptanceTests/README.md),
pseudocode sections 1–15 and shared AT-C01–C04 were reviewed. Fixtures retain
that revision under test_support; Transport's original fixed bytes remain literal
upstream vectors. This is direct in-process Identity with protected Linux storage.

| Case | Executed evidence and outcome |
| --- | --- |
| ID-01 | id::tests::canonical_ids_and_input_order_match_vectors and malformed_claims_selectors_and_weak_keys_fail: exact hash input/digest, selectors and lengths pass. |
| ID-02 | text::tests::fixed_text_and_bit_boundaries_are_canonical: both pinned textual IDs and binary equality across ASCII casing pass. |
| ID-03 | text::tests::parser_rejects_ambiguity_without_trimming: prefix, width, padding, whitespace, alphabet, Unicode and unused-bit rejection pass. |
| ID-04 | creation_checks_authority_and_commits_complete_association, Cryptography identity_keys fixture tests and identity_persistence: independent entropy requests, complete binding and exact restart credentials pass; no network dependency exists. |
| ID-05 | creation_failures_erase_staging_without_automatic_replacement, loading_validates_without_replacement and filesystem cut tests: no usable incomplete authority or silent replacement. Atomic pair records represent individual-key write failures as complete-record failures; native binding is infallible for valid fixed-size keys. |
| ID-06 | proof::tests and id::tests: relation-only validation, wrong key/ID/transcript/domain/role and dependency failures pass. Adapter rejection tests assert no session publication. |
| ID-07 | contact::tests::exact_contact_payloads_and_endpoint_boundaries, query_validation_and_one_time_decoding and composition::identity rejection test: canonical QR/contact values, one-time decoding and expected-peer enforcement pass. |
| ID-08 | contact parser tests reject invalid decimal ports, duplicate/unknown fields, fragments, DNS, brackets, zones and double encoding. |
| ID-09 | provider signing/decryption/backup tests and identity_privacy: scoped denial, exact bytes, private-data exclusion pass. Identity has no Ledger dependency or credit/trust mutation output. |
| ID-10 | identity_recovery and signing_only_recovery_cannot_claim_old_decryption_or_inherited_identity: separate keys/catalog restore, old-object decryption, signing-only failure and new-ID boundary pass. |
| AT-C01 | Scripted Cryptography/KeyStore contracts plus real protected store and Transport adapter pass valid and denied cases. No wire pointer/path. Serialized process-separated adapter is unsupported. |
| AT-C02 | Configured text/contact/sign/envelope/record bounds and dependency scripts reject excess/unavailable work; atomic cut tests preserve OutcomeUnknown and explicit reconciliation. |
| AT-C03 | Contact snapshots remain copied across updates; foreign/expired/revoked authority fails; fresh-process persistence and recovery retain acknowledged complete records. No service obligations or replay database belong to Identity. |
| AT-C04 | Partial/corrupt records and malformed objects fail before usable output; identity_privacy captures TRACE fields; synchronous cancellation scope is documented in LIFECYCLE.md. No hidden network or optional module calls exist. |

Additional section-15 cases map to selection tests (zero/one/multiple configuration,
same ID with competing valid encryption bindings, copied pairs and new signing IDs),
proof tests (signature/profile negatives), and text/contact tests (canonical forms).
Adopted canonical/small-order signature profile and binding/envelope vectors remain
covered by the complete Cryptography suite. See [lifecycle evidence](LIFECYCLE.md)
for exact fallible acquisition/release boundaries and native-backend limitations.

Reproduce from the root with `cargo test --locked`, including doctests, both real
Identity restart suites and composition handshake tests. All listed scenarios ran
on Linux. Unsupported platform adapters return Unsupported; no other platform
conformance is claimed. Fresh processes use the same implementation, so this is
not cross-implementation private-format portability evidence. No serialized Identity
adapter, full socket node, Ledger/Board state recovery, paid remote retrieval,
minimum-profile node or full-feature conformance is claimed. Those require later
roadmap milestones. No new language-independent protocol defect was confirmed in
this audit; previously reported upstream fixes remain tracked in CR2SE bugs.
