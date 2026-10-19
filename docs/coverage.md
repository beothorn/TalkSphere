# Production line coverage

Run coverage from the repository root with stable Rust, the matching LLVM tools,
and [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov):

```sh
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
cargo llvm-cov --locked --lcov --output-path target/production-coverage.lcov \
  --ignore-filename-regex '(/tests/|/tests.rs|/test_support/)'
```

Coverage runs the tests under instrumentation; it does not replace the normal
format, Clippy, test, build and help checks. Reports stay in ignored `target/`.
The filename exclusions remove integration tests, separate unit-test files and
dependency substitutes from the metric, while still executing those tests.

LLVM also reports test code embedded in production source files. Exclude the
inline `#[cfg(test)]` modules when calculating production line coverage. The
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
