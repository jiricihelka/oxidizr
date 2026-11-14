# AGENTS.md

## Project

**oxidizr** is a Rust-based alternative to [Structurizr](https://structurizr.com), the
"models-as-code" architecture tool built around the [C4 model](https://c4model.com).

The core promise is **full compatibility** with the two Structurizr file formats:

- **Structurizr DSL** (`.dsl`) — the indentation-based workspace definition language.
- **Structurizr JSON** (`.json`) — the open JSON workspace format used for exchange with
  Structurizr tooling (Lite, Web/WebDAV, export tooling, etc.).

Workspaces produced by oxidizr must be consumable by the official Structurizr tooling and
vice versa, byte-comparable at the JSON level where the format defines it.

**Current status:** greenfield. The repo contains only this file, `plan/`, and git.
Scaffolding the Cargo workspace (below) and writing design docs into `plan/` is the first task.

## Repository layout

- `plan/` — research notes, design documents and architecture decisions (start with
  `plan/README.md` — it indexes the distilled structurizr reference: `FEATURES.md`,
  `DSL.md`, `JSON-FORMAT.md`, `FIXTURES.md`, `STRUCTURIZR.md`) plus the decision
  registers `DEVIATIONS.md` and `ADDITIONAL_FEATURES.md` and the phase roadmap
  `PLAN.md`. Per-phase implementation plans with all architectural decisions live in
  `plan/build/PHASE_N.md`. Read everything here before making significant design
  decisions; write a short plan doc there when introducing or changing an architectural
  decision.
- `crates/` — Cargo workspace members (to be created):
  - `oxidizr-core` — the domain model: workspace, people, software systems, containers,
    components, relationships, deployment nodes/instances, views, styles, configuration.
    No parsing or serialization dependencies beyond what the model itself needs.
  - `oxidizr-dsl` — DSL lexer/parser → `oxidizr-core` workspace, plus validation.
  - `oxidizr-json` — serde (de)serialization of the Structurizr JSON format.
  - `oxidizr-export` — (later) diagram exporters (PlantUML, Mermaid, D2, ...). Not in
    scope for the first milestones.
  - `oxidizr-cli` — binary crate: `dsl → json`, `json → json`, validation, diffing.

## Commands

Until the workspace is scaffolded these will fail; afterwards:

```sh
cargo fmt --all                            # format
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                     # Category A only (no structurizr bin needed)
cargo bench --workspace                    # criterion benches
cargo xtask oracle verify                  # Category B (needs $OXIDZR_STRUCTURIZR_BIN)
cargo llvm-cov --workspace                 # coverage report (CI, report only)
cargo run -p oxidizr-cli -- dsl2json workspace.dsl -o workspace.json
```

## Source of truth for compatibility

When behavior is ambiguous, the official tooling defines it — do not guess:

- Distilled quick reference: everything in `plan/` (notes below are summarized there).
- Docs: [docs.structurizr.com/dsl](https://docs.structurizr.com/dsl) (language guide,
  keywords, semantics — `dsl/71-language.md` in the docs repo is the authoritative
  grammar page), [docs.structurizr.com/workspaces/file-types](https://docs.structurizr.com/workspaces/file-types),
  [docs.structurizr.com/commands](https://docs.structurizr.com/commands). Docs source:
  [github.com/structurizr/structurizr.github.io](https://github.com/structurizr/structurizr.github.io).
- Reference implementation (Java monorepo, Apache-2.0, v6+):
  [github.com/structurizr/structurizr](https://github.com/structurizr/structurizr) —
  study `structurizr-core` (domain model), `structurizr-client` (JSON reader/writer —
  the actual serialization rules), `structurizr-dsl` (parser), `structurizr-export`.
  The `structurizr/java`, `/cli`, `/lite`, `/ui` repos are archived stubs from the
  pre-v6 era; use the monorepo.
- JSON schema: `structurizr-json/structurizr.yaml` in the monorepo (OpenAPI 3.0) — note
  it **lags the Java code**; the code is the truth.
- DSL/JSON test fixtures: `structurizr-dsl/src/test/resources` (107 `.dsl` fixtures) and
  `structurizr-client/src/test/resources` (back-compat workspaces). There is no public
  `structurizr/samples` repo.

Some artifacts on Maven Central are older GPL-licensed versions — when studying or
vendoring, check the license of the specific version you touch.

## Compatibility rules (hard requirements)

- **JSON is the contract.** Field names, structure, and types must match the Structurizr
  JSON format exactly (camelCase keys as defined by the schema, not by Rust convention).
  Never rename, reorder semantics, or "improve" fields.
- Reproduce reference behaviors, e.g.: element and relationship IDs drawn from ONE
  sequential integer counter in definition order, serialized as strings in JSON
  (`workspace.id` is the only numeric ID); implicit tag stacks (`Element`,
  `Relationship`, `Person`, `Software System`, `Container`, ...) — details in
  `plan/FEATURES.md`; DSL source retention rules (`structurizr.dsl` property);
  verify each of these against the reference implementation, not against memory.
  Note the v6 model REMOVED things still seen in older docs/schema (`location`,
  `enterprise`, `branding`, `env()`, `uses`, `deploy {}` scope) — support them only as
  tolerant errors/reads, not as features.
- DSL support must cover the full grammar: `workspace`/`model`/`views`/`configuration`
  scopes, scoped definitions (people, systems, containers, components, groups),
  relationship statements (incl. `this`, implicit source, archetype arrows `--x->`,
  `-/->` removal, implied relationships), archetypes, `deploymentEnvironment` scopes
  with instances/health checks, `!include`, `!const`/`!var` with `${VAR}` substitution
  (env-var fallback), `workspace extends`, `!docs`/`!adrs`, `!elements`/`!relationships`
  finders, `scope shared`, `!identifiers hierarchical`, styles, themes, filtered views,
  dynamic views (incl. `parallel`), custom/image views, auto-layout, text blocks,
  comments (`//`, `#`, `/* */`) and line continuations.
- Parser errors must carry file, line and the offending source line (the reference
  emits no column; adding one is fine, omitting line info is not).

## Test data & goldens

- `testdata/` holds vendored `.dsl` fixtures (start with the reference test fixtures in
  `structurizr-dsl/src/test/resources` and the golden workspaces in
  `structurizr-export`/`structurizr-client` tests, keeping source and Apache-2.0
  attribution noted in a README) and their golden `.json` files.
- Goldens are generated with the official `structurizr export -format json` command
  (unified application binary; the old `structurizr-cli` is archived; see
  [docs.structurizr.com/commands](https://docs.structurizr.com/commands)). Record the
  tool version used in `testdata/README.md`.
- Category A/B split and the `$OXIDZR_STRUCTURIZR_BIN` discovery/skip rules are defined
  in §Testing — that section governs; this one defines the corpus.
- Round-trip tests: JSON → model → JSON must be lossless for the fields Structurizr
  itself preserves.

## Code quality guidelines

Priorities: **portability, reliability, performance** — and never any of them at the
expense of the compatibility contract.

- Performance: minimize allocations — zero-copy lexing over `&str`, reused buffers,
  `Arc<str>`/small-string handling only where measured; lean on Rust's ownership model
  (moves, borrowing, newtypes, iterators over index loops). No `clone()` on hot paths
  without benchmark evidence; perf claims must be backed by criterion numbers
  (§Benchmarks).
- Reliability: no `unsafe` — any exception needs an audit note in the code and explicit
  sign-off first. Handle errors as values, not control flow (panics abort user work).
- Portability: std-only; no `libc`/platform APIs; every `#[cfg(target_…)]` needs a
  justifying comment. Builds + Category A tests must pass on Linux, macOS and Windows
  (CI matrix). Path/encoding/locale differences are test territory, not `cfg` territory.
- Toolchain: **nightly is pinned** in `rust-toolchain.toml`. Use only reasonably stable
  nightly features (active stabilization path or long incubation — no specialization-
  class megafeatures); every usage is recorded with a one-line rationale in
  `NIGHTLY.md`, re-checked for stabilization at each milestone.

## Working style: when to ask

- Planning phase: ask lots of questions — every decision gets recorded with rationale
  in `plan/` (this file's registers are the output of that process).
- Development phase: implement per the plan without stopping to ask; break the silence
  ONLY when reality contradicts a plan assumption — a V-n verification item comes out
  differently, the reference behaves surprisingly, or a compat gate fails in a way
  PHASE_1.md did not anticipate.

## Testing

Every test belongs to exactly one of two categories:

- **Category A — pure Rust** (default `cargo test --workspace`): unit, integration,
  snapshot (`expect-test`), golden byte-diffs against `testdata/golden/`, property
  tests (`proptest`), read-corpus loads. Must never need network, Java or Docker.
  Runs on the 3-OS CI matrix.
- **Category B — reference-backed**: needs the official `structurizr` binary (golden
  (re)generation via `cargo xtask oracle`, oracle verification, behavioral
  tie-breakers). The binary is located via `$OXIDZR_STRUCTURIZR_BIN`, falling back to
  `structurizr` on PATH; when absent these tests **skip with a loud summary line** —
  except in the dedicated CI job, where a skip is a failure (coverage may not silently
  vanish).

Use all test kinds where they pay off: unit tests for rules (tags, id counter,
canonical names, validation), snapshot tests for lexer/parser shapes and diagnostics,
golden byte-diffs for the JSON contract, property tests for invariants (fmt
idempotence, parse↔print, round-trips), fuzzing (`cargo-fuzz`, nightly) on the
lexer/parser against the fixture corpus, and `cargo-llvm-cov` in CI as a **report
only** — no percentage gate. Dev-only deps: `proptest`/`arbitrary`, `criterion`,
`cargo-fuzz`; no mutation testing (poor ROI here).

## Benchmarks

`criterion` benches live in `benches/` per crate; mandated targets: DSL parse (full
fixture corpus + worst-case per feature), JSON read, JSON write, `fmt`, `reconcile`.
Baselines are committed at milestones M0/M3/M4/M7; compare with `critcmp`. An
optimization is accepted only with before/after numbers; >~10% regression against the
committed baseline is flagged by the CI bench job for human review (no auto-fail).

## Code style

- Rust edition 2024, `rustfmt` defaults, clippy clean with `-D warnings`.
- Library crates use `thiserror` error types; the CLI uses `anyhow`. Parser diagnostics
  carry source spans.
- Domain model types mirror Structurizr concepts (names like `SoftwareSystem`,
  `DeploymentNode`, `ViewKey`) so code maps 1:1 onto the spec/docs; serde renames map
  them to the JSON schema's field names.
- `unwrap()`/`panic!` only in tests. Public items get rustdoc comments.
- Add unit tests next to the code under test; compatibility tests live in the golden
  harness.

## Notes for agents

- `plan/DEVIATIONS.md` lists the agreed, compatibility-safe ways oxidizr differs from
  the reference (diagnostics, rejected JVM-only directives, opt-in flags) and a
  "Locked" list that must never deviate. Check it before implementing any behavior, and
  record any newly proposed deviation there first.
- Purely additive features (F1–F8: reconcile, fmt, diff, oracle, lsp, query, init,
  `--strip-volatile`; plus ideas awaiting sign-off) are registered in
  `plan/ADDITIONAL_FEATURES.md` — check it before building anything that Structurizr
  doesn't have, and get sign-off for 💡 ideas before implementing them.
- First PR in an empty area: scaffold exactly the layout above; don't add exporter or
  server functionality that wasn't asked for.
- Never "fix" a compatibility mismatch by changing the expected Structurizr behavior —
  change oxidizr to match the reference. If the reference itself looks wrong, capture
  the divergence in a `plan/` doc before deviating.
