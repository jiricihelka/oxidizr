# TESTING.md — test architecture & benchmarks

Two-category discipline, the golden corpus policy, and bench rules. What the corpus
actually contains: `testdata/README.md`. Harness pattern: `agents/notes/testing/`.

## Categories (every test belongs to exactly one)

- **Category A — pure Rust** (default `cargo test --workspace`): unit, integration,
  snapshot (`expect-test`), golden byte-diffs against `testdata/golden/`, property
  tests (`proptest`), read-corpus loads. Must never need network, Java or Docker.
  Runs on the 3-OS CI matrix.
- **Category B — reference-backed**: needs the official `structurizr` binary (golden
  (re)generation via `cargo xtask oracle`, oracle verification, behavioral
  tie-breakers). The reference tool is **slow — use it minimally**: these tests are
  compiled and run only behind a default-OFF `reference` feature, and goldens are
  committed so per-PR CI never invokes the binary. The binary is located via
  `$OXIDZR_STRUCTURIZR_BIN`, falling back to `structurizr` on PATH (the nix dev flake
  provides it); when absent these tests **skip with a loud summary line** — except in
  the dedicated CI job (feature enabled), where a skip is a failure (coverage may not
  silently vanish).

Use all test kinds where they pay off: unit tests for rules (tags, id counter,
canonical names, validation), snapshot tests for lexer/parser shapes and diagnostics,
golden byte-diffs for the JSON contract, property tests for invariants (fmt
idempotence, parse↔print, round-trips), fuzzing (`cargo-fuzz`, nightly) on the
lexer/parser against the fixture corpus, and `cargo-llvm-cov` in CI as a **report
only** — no percentage gate. Dev-only deps: `proptest`/`arbitrary`, `criterion`,
`cargo-fuzz`; no mutation testing (poor ROI here).

## Test data & goldens

- `testdata/` holds vendored `.dsl` fixtures (reference test resources in
  `structurizr-dsl/src/test/resources` and the golden workspaces in
  `structurizr-export`/`structurizr-client` tests, source + Apache-2.0 attribution
  kept in `testdata/README.md`) and their golden `.json` files. The corpus is a byte
  contract — `.gitattributes` pins EOL (`agents/notes/testing/byte-exact-corpus.md`).
- Goldens are generated with the official unified application binary:
  `structurizr export --workspace <file> --format json --output <dir>` — GNU-style
  long options only (the unified binary rejects the old CLI's single-dash `-format`
  form; verified against 2026.10.26; the old `structurizr-cli` is archived).
  Driver: `cargo xtask oracle gen|verify`. Record the tool version in
  `testdata/README.md`. Goldens are **committed**; regenerate sparingly (fixture
  changes, milestones) — per-PR CI never calls the reference tool.
- Round-trip tests: JSON → model → JSON must be lossless for the fields Structurizr
  itself preserves (see DEVIATIONS.md D3 for the reference-exact default).

## Benchmarks

`criterion` benches live in `benches/` per crate; mandated targets: DSL parse (full
fixture corpus + worst-case per feature), JSON read, JSON write, `fmt`, `reconcile`.
Baselines are committed at milestones M0/M3/M4/M7; compare with `critcmp`. An
optimization is accepted only with before/after numbers; >~10% regression against the
committed baseline is flagged by the CI bench job for human review (no auto-fail).
Run via `cargo xtask bench` — never forward args into `cargo bench --workspace --`
(`agents/notes/pitfalls/cargo-bench-args.md`).
