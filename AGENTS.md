# TalkSphere contributor instructions

## Purpose and scope

TalkSphere is a Rust server implementation and proof of concept for
[CR2SE](https://github.com/beothorn/CR2SE). Implement it module by module, from
the basic modules through a working node and then the remaining planned
capabilities. Keep minimum-profile and full-feature coverage distinct.

Use GitHub URLs when referring to CR2SE in code, documentation, reports and
fixtures. Never record a developer's local CR2SE checkout path. Consult the
upstream specification, module input/output contracts, pseudocode, acceptance
tests and test vectors before implementing a module. Record the upstream
revision used for behavior and fixtures; investigate contradictions rather
than silently inventing wire behavior.

## Planning method

- Maintain [plans/PLAN.md](plans/PLAN.md) as the main roadmap checklist, retaining
  completed setup and the remaining module milestones.
- Break down the module currently requested into its own
  `plans/<number>_<Module_name>/PLAN.md`; the first is
  [plans/1_Transport_module/PLAN.md](plans/1_Transport_module/PLAN.md).
  Folder numbers count the modules being planned, so Transport is folder 1
  while remaining step 2 after setup in the main roadmap.
- Start each detailed plan with a numbered checklist whose entries are each
  exactly one short, actionable sentence describing one manageable step.
  Keep the sentences easy to review without reading the technical details.
- Put explanations, dependencies, acceptance criteria and testing instructions
  in a separate details section below the complete checklist, in the same file.
  Number those detail sections to match the checklist and keep the same order;
  do not bury extra steps in checklist sub-bullets or multi-sentence entries.
- Size steps as focused changes that can be implemented and reviewed separately;
  include unit tests with each implementation step rather than postponing all
  testing to the end.
- Expand only the requested module, preserving the scope and progress of the
  other roadmap entries, and link the detailed plan from the main roadmap.
- For a planning-only request, leave proposed implementation steps unchecked
  and stop after preparing the plan for review; wait for a subsequent request
  to implement it instead of treating creation of the plan as authorization.
- When moving or renaming plans, update README links, contributor instructions
  and plan paths in CLI help/diagnostics so the plan remains discoverable.
- Mark work complete only after its implementation, tests, help and
  documentation are complete; a scaffold does not complete a module.

## Module boundaries

- Keep protocol code under `src/modules/<module>/`, one directory per CR2SE
  module. Use Rust names such as `peer_protocol` and `node_api` where needed.
- Split each module into focused files and, when useful, subfolders for codecs,
  state, adapters and tests. Create folders when they contain useful work;
  avoid empty speculative hierarchies or a large catch-all file.
- Keep `src/main.rs` thin, CLI behavior in `src/cli/`, and composition outside
  protocol internals. Modules must use explicit contracts rather than another
  module's private state. Inject platform and dependency substitutes for tests.
- Transport can be developed before Identity and Cryptography using test-only
  substitutes. Never expose those substitutes as authenticated networking.
- An unfinished capability must return an explicit error and must not be
  advertised as supported. Preserve protocol errors, bounds and exact values.

## Rust development and organization

These conventions adapt the code-focused guidance in
[coisaCommander's contributor guide](https://github.com/beothorn/coisaCommander/blob/main/AGENTS.md)
to Rust and TalkSphere's module boundaries.

- Prefer simple, idiomatic Rust and the smallest clear implementation. Split
  responsibilities into focused functions, types and files; avoid speculative
  abstractions, unnecessary dependencies and deeply nested control flow.
  Look for opportunities to simplify the code touched by a change.
- Organize by protocol domain first, then by responsibility within that domain
  (for example `transport/codec/`, not a global `codecs/transport/` tree).
  Put shared contracts at the nearest common module boundary and specialized
  implementations below it. Folder structure should make a feature easy to find;
  keep dependency direction explicit rather than inferring it from folder depth.
- Present public types, traits and commonly used operations before private
  helpers where practical. Keep internals private and expose only the contract
  callers need; a caller should understand usage without reading implementation
  details. Use `pub(crate)` or narrower visibility for internal collaboration.
- Consult Git history, `git blame` and the relevant plan details when existing
  behavior is unclear. Preserve intentional behavior unless the task calls for
  changing it; document the reason for meaningful design choices.
- Use clear names and named constants or local bindings for values whose meaning
  would otherwise be hidden in a call. Include units in names where useful.
  Prefer immutable bindings; add `mut` only when needed and explicit types when
  they clarify a contract or prevent ambiguous numeric interpretation.
- Prefer guard clauses, early returns, `?`, `let ... else` and exhaustive `match`
  expressions to deeply nested branches. Return typed `Result` errors for
  expected failures instead of panicking or using unchecked `unwrap()`.
- Use iterator adapters when they clarify transformations; separate meaningful
  stages into small maps or named helpers. Prefer a straightforward loop when
  iterator chains obscure state, errors or ownership. Do not introduce clones,
  boxing or extra collections merely to force a functional style.
- Keep signatures and expressions readable and reasonably narrow. Format
  multiline parameter lists as parenthesized blocks with one parameter per
  line, a trailing comma and the closing parenthesis aligned with the function
  declaration. Let `cargo fmt` determine wrapping and indentation; do not fight
  rustfmt with manual alignment or formatting exclusions. Explain parameters in
  rustdoc, including their purpose, origin, units and constraints when relevant.

## Comments and contract documentation

- Begin each source module with `//!` documentation explaining its responsibility,
  callers and dependencies. Use `///` for public types, traits, functions and
  fields so the generated API documentation explains how to use them.
- Place a human-readable explanation before every function or method, including
  private helpers and tests: use rustdoc for public APIs and ordinary comments
  for internal behavior. Describe the purpose, inputs, result, important rules
  and failure behavior in enough detail to reconstruct the intended behavior.
  For tests, describe the scenario and expected outcome.
- Write comments as an explanation to a developer learning the code. Explain
  why a choice exists, where values come from, and surprising ownership,
  concurrency or protocol constraints. Avoid comments that merely repeat a
  variable name or narrate obvious syntax. Keep explanations current when code
  changes; focus on observable behavior rather than a line-by-line translation.
- Document public contracts with relevant `# Errors`, `# Panics` and usage
  examples. State ownership/lifetime expectations and side effects where they
  matter; use executable rustdoc examples for public usage when practical.
- For each Rust source file defining a public contract, maintain a companion
  Markdown file with the same stem (for example `header.rs` and `header.md`).
  Describe public types, traits, constructors, functions and fields in plain
  language or language-independent pseudocode, including inputs, outputs and
  failure rules. Omit private implementation details and Rust implementation
  listings so a programmer unfamiliar with Rust can understand the contract.
  Update the companion document whenever the public contract changes; test-only
  files and files containing only private helpers do not need companions.
- Each module README must also describe its callers, dependencies, relationships
  to other modules and the reasons for its responsibility boundaries.

## Logging

- Use a shared structured logging framework such as `tracing`, with the
  subscriber configured by application composition rather than protocol code.
  Include module/function context and support filtering by target and level.
  Send operational logs to stderr; preserve stdout for intentional CLI output.
- Add a trace event or trace-level span at the start of production functions
  to make execution flow discoverable. Keep logging out of logging/formatting
  implementations where it could recurse; test functions need scenario comments,
  not entry logs. Use narrative messages that explain the operation or transition.
- Choose levels by audience and consequence: `trace` explains developer control
  flow; `debug` adds safe values useful for diagnosis; `info` marks a small number
  of significant normal lifecycle events; `warn` explains an unexpected condition
  the operation can recover from; `error` explains a failed operation and its
  consequences when the application can continue. For an unrecoverable application
  failure, emit `error` and perform controlled shutdown with a failing exit status;
  Rust's common logging frameworks do not provide a separate `fatal` level.
- Keep `info` sparse. Warn/error messages should explain what went wrong, its
  effect and any recovery action. Avoid logging the same failure at every layer;
  log it at the boundary that handles it, preserving structured error context.
- Put diagnostic values in structured fields and make expensive debug/trace
  preparation conditional on the level being enabled. Never log private keys,
  tokens, secret material or raw application payloads. Do not record unauthenticated
  peer input as trusted facts; bound and sanitize any peer-controlled diagnostic
  fields and avoid unbounded log volume on per-byte or repeated failure paths.
- When adding logging configuration or CLI options, document defaults and filters
  and update overview/detailed help and CLI tests together.

## Protocol bug reports

Record confirmed CR2SE issues as individual Markdown files in
[`CR2SE bugs/`](CR2SE%20bugs/README.md), following its template and numbering.
Report language-independent defects in the protocol, contracts, pseudocode,
vectors or acceptance tests, with upstream revision/section links, evidence,
impact and a proposed clarification or correction. Rust tooling, ownership,
crate or implementation problems are local issues, not CR2SE bugs. Do not
fabricate reports to populate the directory. Keep documented workarounds
separate from the normative protocol, and update reports when upstream fixes
them.

## Tests, documentation and help

- Always add or update unit tests with implementation changes, including module
  scaffolds. Test observable behavior and relevant invalid/failure paths.
  Add integration tests for executable behavior and cross-module contracts.
- Cover happy paths and ask what can go wrong: include invalid inputs, boundary
  values, dependency failures and recovery behavior where applicable. Include
  both focused technical tests and realistic caller scenarios; explain the
  scenario in a comment instead of relying only on the test function's name.
- Fix failing tests and newly introduced compiler/Clippy warnings before marking
  implementation complete; do not disable tests or weaken assertions to hide a
  regression.
- Use upstream acceptance cases and fixed vectors as implementation progresses;
  do not claim conformance based only on local round trips.
- Keep the root README's build, test and run instructions current. Each module
  README must explain its purpose, status, upstream links and how to build,
  run/exercise and test it from the repository root.
- Keep `talksphere --help` a concise overview. Every command and option must
  have discoverable detailed help through `talksphere help <topic>` (including
  option spellings, for example `talksphere help --version`). Update the shared
  help definitions, parser, README examples and CLI tests together.
- No arguments show help successfully. Invalid arguments exit with status 2;
  runtime failures exit with status 1 and explain the problem on stderr.
- Keep dependencies deliberate and commit Cargo.lock. Never commit keys,
  tokens, runtime state, logs or build artifacts.

For implementation changes, keep production line coverage above 80%, measure it
with a Rust coverage tool such as `cargo llvm-cov --locked` using the
[production coverage procedure](docs/coverage.md) to exclude test-only code, and report the measured
percentage or explicitly state why coverage could not be measured.

Before finishing a change, run these commands from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
cargo run --locked -- --help
```

Also exercise any changed commands and detailed help topics. Fix failures or
state clearly what could not be verified and why.
