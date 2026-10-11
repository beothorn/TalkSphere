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
