# WORKFLOW.md — environment, commands, code rules

Permanent dev/product workflow. Operational specifics with commands that bit us live
in `agents/notes/` (env/, pitfalls/); this file is the policy and the full command map.

## Dev environment

- Development happens **in nix (flake.nix)**: the dev shell pins the nightly Rust
  toolchain, a JDK and the pinned `structurizr` reference binary (for Category B work).
  CI and end users need no nix — plain `cargo` must build and pass Category A on the
  3-OS matrix without it. See `agents/notes/env/nix-flake.md` and the Wine leg note.
- Toolchain: **nightly is pinned** in `rust-toolchain.toml`. Use only reasonably stable
  nightly features (active stabilization path or long incubation — no specialization-
  class megafeatures); every usage is recorded with a one-line rationale in
  `NIGHTLY.md`, re-checked for stabilization at each milestone.

## CLI/config policy (product rule)

- **No config files.** Flags + env vars only. Env family: `OXIDZR_STRUCTURIZR_BIN`,
  `OXIDZR_DOT_BIN`, `OXIDZR_THEMES_DIR`, `NO_COLOR`. One global `--allow-net` gates all
  URL input (theme URLs, `!include <url>`, `workspace extends <url>`) and is never
  settable via env (DEVIATIONS.md D5). `-` = stdin/stdout where a command takes a file argument.
- Shipped binaries: `oxidizr` (CLI), `oxidizr-viewer` (Phase 3 GUI), `oxidizr-lsp`
  (Phase 4).
- License: **MIT OR Apache-2.0** dual (LICENSE-MIT + LICENSE-APACHE at root, workspace
  metadata); library crates publish to crates.io; vendored fixtures keep their
  Apache-2.0 attribution (`testdata/README.md`).

## Commands

```sh
cargo fmt --all                            # format
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                     # Category A only (no structurizr bin needed)
cargo test --workspace --features reference  # + Category B (slow — run sparingly)
cargo bench --workspace                    # criterion benches (no arg forwarding!) — skips
                                           # the e2e corpus benches (feature-gated, local-only)
cargo xtask ci                             # the commit gates locally: fmt + clippy + test + refs
cargo xtask ci windows                     # + the Wine windows-gnu leg (nix dev shell)
cargo xtask ci gui                         # + GUI clippy legs: viewer shell + the in-process
                                           # `oxidizr view` entry (§P3-M8, both packages)
cargo xtask refs                           # doc path & §section reference integrity only
cargo xtask ci lint                        # actionlint on .github/ (CI's lint job is authoritative)
cargo xtask bench                          # bench job mirror + e2e corpus benches
                                           # (--features oxidizr-e2e/e2e-bench; reports: target/criterion)
cargo xtask bench save <LABEL>             # snapshot criterion JSONs into bench-baseline/<LABEL>/
cargo xtask bench cmp <LABEL>              # critcmp baseline vs target/criterion
cargo xtask cov                            # coverage job mirror (summary, report only)
cargo xtask fuzz [SECONDS]                 # fuzz job mirror (nightly CI): all targets, fixture seeds
cargo xtask oracle verify                  # Category B (needs $OXIDZR_STRUCTURIZR_BIN)
cargo llvm-cov --workspace                 # coverage report (CI, report only)
cargo run -p oxidizr-cli -- dsl2json workspace.dsl -o workspace.json
```

## Milestone discipline (adopted 2026-11-15, after M4)

- **Size gate:** a milestone touching more than ~4 modules is decomposed into
  sub-milestones in its `build/PHASE_N.md` entry BEFORE implementation starts
  (M4's single ~15-subsystem commit cost ~10x an average milestone).
- **Commit every green sub-piece** (gate quartet green = commit); the milestone ends
  with a wrap-up commit (PROGRESS.md milestone log + status board update).
- **Keep `agents/notes/RESUME.md` current** — rewritten at every green commit;
  sessions/compactions resume from it instead of re-deriving state
  (`agents/notes/process/resume-ritual.md`).

## Execution model (harness facts, adopted 2026-11-15)

The harness machine is powerful and effectively single-tenant (many parallel
background jobs are cheap); the LLM server serves **at most one agent at a
time**. Consequences for how work is run — detail in
`agents/notes/process/parallelism.md`:

- Offload to local tools whatever is deterministic (gates, `refs`, `lint`,
  ast-grep audits) — LLM turns are the scarce resource, harness cycles are not.
- **Subagents are near-useless here**: they serialise on the single LLM slot,
  so they add overhead without parallelism.
- Launch long independent harness jobs (nix builds, oracle/bench/cov runs,
  file analysis) in the background early; never serialise them behind turns.
- Two `cargo` jobs do NOT parallelise (shared target-dir lock) — chain cargo
  work in one command; reserve parallelism for non-cargo resources.

## Code quality

Priorities: **portability, reliability, performance** — and never any of them at the
expense of the compatibility contract (`COMPAT.md`).

- Performance: minimize allocations — zero-copy lexing over `&str`, reused buffers,
  `Arc<str>`/small-string handling only where measured; lean on Rust's ownership model
  (moves, borrowing, newtypes, iterators over index loops). No `clone()` on hot paths
  without benchmark evidence; perf claims must be backed by criterion numbers
  (`TESTING.md` §Benchmarks).
- Reliability: no `unsafe` — any exception needs an audit note in the code and explicit
  sign-off first. Handle errors as values, not control flow (panics abort user work).
- Portability: std-only; no `libc`/platform APIs; every `#[cfg(target_…)]` needs a
  justifying comment. Builds + Category A tests must pass on Linux, macOS and Windows
  (CI matrix). Path/encoding/locale differences are test territory, not `cfg` territory.

## Code style

- Rust edition 2024, `rustfmt` defaults, clippy clean with `-D warnings`. The workspace
  lint bar is clippy `all + pedantic` plus the 2R ratchet `redundant_clone` (nursery =
  nightly-only: `NIGHTLY.md` register row, keep verdict + measurements in
  `build/PHASE_2R.md` §D4).
- Library crates use `thiserror` error types; the CLI uses `anyhow`. Parser diagnostics
  carry source spans.
- Domain model types mirror Structurizr concepts (names like `SoftwareSystem`,
  `DeploymentNode`, `ViewKey`) so code maps 1:1 onto the spec/docs; serde renames map
  them to the JSON schema's field names.
- `unwrap()`/`panic!` only in tests. Public items get rustdoc comments.
- Add unit tests next to the code under test; compatibility tests live in the golden
  harness.
- Lint suppressions for faithful-port reasons need an inline rationale comment
  (`agents/notes/process/faithful-port.md`).
- Recorded crate/test conventions (2R R-core-13 ratification, not faithful-port
  suppressions): core `builder.rs`/`viewops.rs` carry module-scope
  `#![allow(missing_errors_doc, missing_panics_doc)]` — failure modes are enumerated by
  the shared `CoreError` enum, not repeated per method; and every test module carries
  `#![allow(clippy::unwrap_used)]` (28 files — the `unwrap`-only-in-tests charter's
  guard), so a test-module `unwrap` needs no new allow.
