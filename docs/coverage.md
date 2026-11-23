# Production line coverage

Run coverage from the repository root with stable Rust, the matching LLVM tools,
and [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov):

```sh
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
cargo llvm-cov --locked --failure-mode all --lcov --output-path target/production-coverage.lcov \
  --ignore-filename-regex '(/tests/|/tests.rs|/test_support/)'
```

The subprocess suites intentionally exercise abrupt termination. Such a child can
leave an incomplete LLVM profile. `--failure-mode all` allows LLVM to merge the
valid captures while warning about incomplete ones; it still fails if none can
be merged. This does not ignore test failures or remove production source from
the denominator. Events present only in a discarded child capture contribute no
coverage, so report the result from valid profiles and retain the warning log.

Coverage runs the tests under instrumentation; it does not replace the normal
format, Clippy, test, build and help checks. Reports stay in ignored `target/`.
The filename exclusions remove integration tests, separate unit-test files and
dependency substitutes from the metric, while still executing those tests.

LLVM also reports test code embedded in production source files. Exclude the
inline `#[cfg(test)]` modules and single-line test-only fields/initializers when
calculating production line coverage. The
following calculation uses LLVM's executable-line counts; it includes uncovered
production lines and excludes neither inconvenient branches nor whole production
files. It matches this repository's top-level inline test/fixture modules, whose
closing braces are at column zero after rustfmt. If test-module layout changes,
review the filter rather than silently assuming its exclusions remain correct.

```sh
python3 - <<'PY'
from pathlib import Path
import re

covered = total = 0
for record in Path('target/production-coverage.lcov').read_text().split('end_of_record'):
    match = re.search(r'^SF:(.*)$', record, re.M)
    if not match:
        continue
    source = Path(match.group(1)).read_text()
    excluded = set()
    for block in re.finditer(
        r'^#\[cfg\(test\)\]\n(?:pub\([^\n]*\) )?mod \w+ \{\n.*?^\}',
        source, re.M | re.S,
    ):
        first = source[:block.start()].count('\n') + 1
        last = source[:block.end()].count('\n') + 1
        excluded.update(range(first, last + 1))
    # Secret lifecycle observers add cfg(test)-only fields and initializers.
    # Match only the currently used single-line field syntax, not production code.
    for field in re.finditer(
        r'^[ \t]*#\[cfg\(test\)\]\n[ \t]+(?:pub(?:\([^)]*\))? )?\w+:[^\n]+,',
        source, re.M,
    ):
        excluded.add(source[:field.end()].count('\n') + 1)
    for line, count in re.findall(r'^DA:(\d+),(\d+)', record, re.M):
        if int(line) not in excluded:
            total += 1
            covered += int(count) > 0
assert total, 'No production coverage records found'
print(f'Production line coverage: {covered}/{total} = {100 * covered / total:.2f}%')
assert covered / total > .8, 'Production line coverage must exceed 80%'
PY
```

Step 4 measured **412/440 executable production lines (93.64%)** after these
exclusions; the new control codecs covered 166/166 lines. This is line coverage,
not proof of branch completeness, protocol conformance or cryptographic validity.
Stable coverage here excludes rustdoc executions; `cargo test --locked` runs
those separately. Always record a fresh result for later implementation changes.

Transport step 24 measured **2833/3011 executable production lines (94.09%)**.
The final audit moved the test-only ID-exhaustion fixture into its excluded test
file; inline test modules and all test-support/fixture files remain excluded.
All 174 unit/integration/documentation tests pass separately. See the
[acceptance matrix](../src/modules/transport/ACCEPTANCE.md) for behavior coverage
and the real-cryptography checks still pending.

Cryptography step 1 measured **2862/3040 executable production lines (94.14%)**.
The new configuration/error implementation accounts for 29 covered production
lines; contract declarations do not execute cryptographic operations. All 183
unit/integration/documentation tests pass separately. This scaffold measurement
does not claim algorithm or envelope conformance.

The Cryptography step 21 audit additionally excludes single-line `cfg(test)`
fields/initializers used by secret-release probes. Historical measurements retain
the procedure used at their recorded step; current measurements use this stricter
filter. Review it if future conditional items use a different layout.

Cryptography completion (step 22) measured **3570/3749 executable production
lines (95.23%)**, using the current exclusions above. All **227** unit,
integration and documentation tests pass separately. Formatting, Clippy with
warnings denied, build, generated API docs and overview/detailed help checks also
pass. See [Cryptography acceptance evidence](../src/modules/cryptography/ACCEPTANCE.md)
for fixed vectors, limits, lifecycle coverage and the Identity integration handoff.

Peer Protocol completion (step38,2026-11-14) measured
**20026/21376 executable production lines (93.68%)**, with the documented
test-only source/inline exclusions. Final verification includes the required
formatting, strict all-target Clippy, locked test/build/help commands, all examples,
actual authenticated custom-node runs, bounded process/issuer restart scenarios
and warning-free API documentation. See the
[Peer Protocol acceptance matrix](../src/modules/peer_protocol/ACCEPTANCE.md) for
independent vector provenance, standard-family substitution scope and the open
paid-check timeout question; coverage percentage alone is not conformance.

Storage step 2 measured **20038/21388 executable production lines (93.69%)**.
Contract admission tests and the complete existing test suite passed.

Storage step 3 changes test-only helpers; production code and its measured
93.69% coverage denominator are unchanged. The full suite and required checks pass.

Storage step 5 measured **20088/21438 executable production lines (93.70%)**.
Exact rational prices pass pinned literals, uint64 extrema and independent
small-rational oracle cases; required checks and the full suite pass.

Storage step 6 measured **20149/21500 executable production lines (93.72%)**.
Offering tests cover both roles, compatible extensions, invalid rates/limits and
advertised maximum-price overflow. Full required checks pass.

Storage steps 7–8 measured **20329/21687 executable production lines (93.74%)**.
Operation validation and lease authorization pass all required checks.

Storage steps 9–10 measured **20448/21802 executable production lines (93.79%)**.
Retention, requester clock policy and concurrent quota tests pass with all
required checks and warning-free API documentation. Durable ownership and network
Storage execution remain later work.

Storage step 11 measured **20682/22042 executable production lines (93.83%)**.
Protected real filesystem staging, exact short writes/reads, collision protection,
retained handles and pre/post-install durability cuts pass with all required checks.

Storage step 12 measured **21053/22422 executable production lines (93.89%)**.
Identity-bound metadata, exact retained outputs/tokens, corruption/count limits,
real file reopen and snapshot durability cuts pass all required checks.

Storage step 13 measured **21214/22587 executable production lines (93.92%)**.
Original-key staging/preparation, concurrent duplicate calls, dependency failures,
unknown-write gating and real protected journal reopen pass all required checks.

Storage step 14 measured **21410/22784 executable production lines (93.97%)**.
Exact create/renew/remove proposals, prepared mutation exclusion, idempotent
terminal decisions, immutable renewal guards and lost terminal replies pass
all required checks. These are local participant tests, not paid node conformance.

Storage step 15 measured **21710/23103 executable production lines (93.97%)**.
Real owner reopen, terminal coordination checks, byte inventory, clock recovery,
quota reconstruction and fail-closed admission pass all required checks and
warning-free rustdoc. The finite profile retains coordinator history and charges
terminal/orphan bytes conservatively until explicit reclamation is implemented.

Storage step 16 measured **21910/23304 executable production lines (94.02%)**.
Complete/unresolved store metadata, provisional capacity refusal, exact pre-upload
prices and backward-compatible metadata-aware Services quote calculation pass
all required checks. Actual paid lease creation remains pending upstream issue #11.

Storage independent steps 18–19 measured **22144/23559 executable production
lines (93.99%)**. Requester result/catalog checks and native scoped range
acceptance pass required checks. Step 17 remains pending upstream issue #11;
these results do not claim a complete paid Storage provider.

Storage step 20 measured **22300/23720 executable production lines (94.01%)**.
Exact range results, cross-chunk integrity scans, original-key journal recovery,
short/corrupt source failures and capacity refusal pass all required checks.
Paid creation and complete network handler composition remain pending.

Storage step 21 measured **22376/23799 executable production lines (94.02%)**.
Requester retrieval metadata, original roles/prices, incomplete output, full-object
hash verification and explicit partial-range integrity limits pass required checks
and warning-free rustdoc. Saved complete values remain independently verifiable.

Storage step 22 measured **22496/23927 executable production lines (94.02%)**.
Authorized renewal assessment, stale/expired guards, exact independent size factors,
checked duration/total limits and uint64-size pricing pass all required checks
and warning-free rustdoc without changing existing lease obligations.

Storage step 23 measured **22669/24105 executable production lines (94.04%)**.
Native renewal staging/reopen, competing preparation threads, renew/remove guard
ordering, pending expiry, confirmed/uncertain persistence and independent requester
validation pass required checks and warning-free rustdoc. Decisions and charging
remain Services-owned; these tests do not claim complete paid network integration.

Storage step 24 measured **22787/24228 executable production lines (94.05%)**.
Logical removal, independent requester checks, accepted read completion, later
inactive-operation refusal, renewal ordering, explicit abort and failed/lost
persistence acknowledgements pass all required checks and warning-free rustdoc.
Physical reclamation and full paid network composition remain later steps.

Storage step 25 measured **22983/24426 executable production lines (94.09%)**.
Terminal expiry, bounded cursor batches, shared byte lifetimes, pinned/orphan-name
accounting, failed deletion and lost metadata/unlink replies pass required checks
and warning-free rustdoc. Coverage exposed an existing Identity test lock-reopen
race; the same bounded post-drop retry used by its atomic tests now covers backup
and restore cuts. The subsequent full verification and coverage run passed.

Storage step 26 measured **23103/24549 executable production lines (94.11%)**.
Separately paid byte evidence, private-field refusal, exact offsets/prices,
accepted pin lifetime, missing/short backing bytes and optional overlap/one-second
frequency boundaries pass required checks and warning-free rustdoc.

Storage step 27 measured **23175/24623 executable production lines (94.12%)**.
Private expected-byte comparisons, exact original send/response/end timing,
malformed/foreign/missing evidence, activity ambiguity and bounded unbiased
sampling pass all required checks and warning-free rustdoc. All 109 Storage
unit tests pass. Step 17 and complete provider/network acceptance remain pending
CR2SE issue #11; no paid Storage provider is advertised.

The independent catalogue portion of Storage step 28 measured
**23284/24741 executable production lines (94.11%)**. Five new tests cover
complete schema tampering, equivalent descriptions and numeric versions, optional
extensions, bounded JSON serialization and real Services Registry admission
without provider support. Required checks and warning-free rustdoc pass. Step 28
remains unchecked pending complete handler installation/publication and step 17.

Storage step 28 requester-context integration measured **23326/24788 executable
production lines (94.10%)**. Context-aware metadata, remote factor validation and
quote pricing preserve the original authorized peer/key in provided and wanted
roles; legacy defaults remain compatible. Required checks and warning-free
rustdoc pass. This local Services port extension changes no protocol wire fields.

Storage step 28 installed-requester integration measured **23806/25298 executable
production lines (94.10%)**. All 129 Storage unit tests and the required repository
checks pass, including scoped exact quotes, original lease snapshots, complete
result validation and paid evidence rules. Rustdoc is warning-free. LLVM discarded
two incomplete abrupt-child captures under the documented failure-mode procedure;
the percentage counts valid profiles with all production source in the denominator.
Requester-only registration enables no provider; step 17 and full step 28 remain
pending upstream acceptance-time clarification.

Storage requester Peer Protocol integration measured **23813/25298 executable
production lines (94.13%)**. Seven new integration tests run actual cached contract
selection, scoped ordinary/check quotes, consent, detached bodies, terminal result
gates and requester Services/Ledger decisions. Required checks and warning-free
rustdoc pass. Remote replies/currentness are controlled and local stores are
volatile here; no provider durability/process or full payment-mode claim is made.

Storage requester payment-mode integration measured **23817/25298 executable
production lines (94.15%)**. Requester issuance and independent signed external
issuer transfer/fee settlement join the provider-currency tests. Exact consent
bounds, uncertain/refused payment, mismatched evidence and duplicate confirmation
pass required checks and warning-free rustdoc. The tests use volatile owners and
controlled remote Storage output; full provider-side policy, actual issuer network
provenance and durable restart coverage remain pending in step 31 and later steps.

Storage wanted-requester integration measured **23818/25298 executable production
lines (94.15%)**. Actual local wanted publication, locally chosen input/spending,
provider-opened stream ordering, finite issuance and duplicate requester commit
pass alongside unauthorized invitations, refusal and publication withdrawal before
or during application choice. Required checks and warning-free rustdoc pass.
These tests install no Storage provider and use controlled remote results;
contribution spend-back and full AT-E02 remain pending under step 30.

Storage native requester recovery measured **23816/25298 executable production
lines (94.14%)**. All 729 library tests and the required repository checks pass,
including sixteen requester integration tests. Four-operation native reopen
boundaries, exact retained result/effect bytes, unknown remote outcomes, denial
of local abort after dispatch and eight independent coordinator/participant
persistence failures pass with fresh authorities and no duplicate charge.
Rustdoc is warning-free and local Markdown links pass. This is local requester
recovery evidence; actual paid provider obligations and complete step 32 remain
pending step 17 and CR2SE issue #11.

Storage native requester/issuer payment recovery measured **23822/25298 executable
production lines (94.17%)**. All 731 library tests, integration/doctests and required
repository checks pass; rustdoc is warning-free and local Markdown links pass.
Requester-issued credits retain finite issuance across all completion boundaries.
Third-party pending/unknown/refused state, exact fee/order binding, independent
native issuer transfer queries after a lost reply and one-time confirmation
survive reopen without changing retained Storage results or trust. These are
requester accounting tests with controlled Storage output and synthetic issuer
provenance; actual provider obligations and complete steps 31/32 remain pending.

Services decision-time finalization measured **23922/25425 executable production
lines (94.09%)**. All 732 library tests and required checks passed. The recovery
fixture covers lost decision writes, original result replay, one-time accounting
and abort on timestamp overflow. LLVM skipped one invalid profile from the
suite under the documented failure-mode procedure.

Storage decision-time lease creation and native provider integration measured
**24529/26135 executable production lines (93.85%)**. All 735 library tests
and required checks passed. The provider fixture exercises Board publication,
all four ordinary operations, exact settlement and native owner stores. LLVM
skipped one invalid profile under the documented failure-mode procedure.

The authenticated Storage exercise checkpoint passed all 735 library tests,
the full contributor checks and all six example tests. The native process
restart, encrypted replica restore, wanted contribution and deliberate
run/help commands succeeded. Production line coverage was **93.85%**
(24529/26135) under the library coverage procedure.

Peer policy completion measured **30079/32462 executable production lines (92.66%)** under the procedure above, with all library,
integration, process and doctest suites passing under instrumentation (893 library tests). The new policy, replica, upkeep and simulation
files are between 91 % and 100 %, except `src/composition/node/wanted.rs` (27 %): its completion handler runs only inside serve
processes whose profiles abrupt-termination tests discard; process tests (`a_client_earns_credits_by_fulfilling_wanted_storage`,
the replication and story tests) exercise it. Run only one coverage job at a time: overlapping jobs share `target/llvm-cov-target` and
delete each other's test binaries, which fails the tests that spawn the current executable.

Messaging (plan 11) keeps its tests in `*_tests.rs` files and `service/crash.rs` beside the source, so measure it with
`--ignore-filename-regex '(/tests/|/tests.rs|/test_support/|/test_support\.rs|_tests\.rs|/crash\.rs)'`. That run measured
**34114/36738 production lines (92.86%)**, **3948/4194 (94.13%)** inside `src/modules/messaging`.

Messaging completion (steps 35-38, with the Identity agreement, shared node identity and four-node exercise) measured
**34358/36989 production lines (92.89%)** for the repository and **4038/4283 (94.28%)** inside `src/modules/messaging`, with the same
exclusion regex; the instrumented run exited 0.

Static Pages and Hosted Publications completion (milestone 15) measured **38864/41741 production lines (93.11%)** for the repository, **3448/3632 (94.93%)** inside
`src/modules/hosted_publications` and **1043/1112 (93.79%)** inside `src/modules/pages`, with the Messaging exclusion regex (`/tests/`, `/tests.rs`, `/test_support/`,
`_tests.rs`, `/crash.rs`); the instrumented run exited 0 and all required checks pass.
