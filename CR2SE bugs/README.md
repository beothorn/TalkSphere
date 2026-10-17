# CR2SE protocol bugs

Keep one confirmed, language-independent issue per Markdown file here so
[CR2SE](https://github.com/beothorn/CR2SE) can be corrected as TalkSphere grows.
Confirmed reports:

- [0001: Diagnostic payload pseudocode accepts a leading byte-order mark](0001-diagnostic-byte-order-mark.md) — open.

Use filenames such as `0001-short-description.md`, choosing the next unused
number, and copy [TEMPLATE.md](TEMPLATE.md). Link the exact upstream revision
and sections. Include a minimal trace, conflicting statements or vector that
another implementation can reproduce, expected/actual behavior, impact and a
proposed resolution. Distinguish evidence from open questions.

Suitable reports include ambiguous wire encodings, contradictory state
transitions, incorrect pseudocode/vectors, missing interoperability rules and
language-independent security or recovery defects. Rust compiler errors,
ownership constraints, crate selection, performance preferences and bugs in
our own code do not belong here. A missing TalkSphere implementation is not a
protocol defect. Do not silently turn a local workaround into a protocol rule.

Maintain each report's status (`open`, `reported upstream`, `resolved`, or
`invalid`) and add upstream issue/fix links when available. Creating a local
report does not automatically file a GitHub issue.

## Build, run and validate

These reports are documentation and need no separate executable. From the
repository root, build with `cargo build --locked`, run the overview with
`cargo run --locked -- --help`, and test with `cargo test --locked`. When a
report has an executable reproducer, put its test in the owning module and
record the exact invocation in the report.
