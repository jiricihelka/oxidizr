# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = T5 commit (tip of `main`, tag `v0.1.0` far down) ·
gates green · nothing pushed.

- **Done:** **PHASE 1 COMPLETE** (M0–M7, tag `v0.1.0` local). **PHASE 1T T0–T5
  DONE**: T0 `oxidizr-api`; T1 harness+seed; T2 valid corpus 156; T3 invalid
  corpus 144 + `xtask e2e mutate`; T4 full-tier invariants (`tests/full.rs`,
  env `OXIDZR_E2E_FULL`) + nightly `e2e.yml` + 2 oracle-fixed engine bugs
  (DSL color normalization, fmt verbatim magic-ends); **T5 (this commit)** =
  six `e2e_` criterion benches in `oxidizr-e2e/benches/e2e.rs` (dsl2json all +
  biggest, json2json, fmt, validate, reconcile) behind cargo feature
  `e2e-bench` with `required-features` on the bench target — CI's plain
  `cargo bench --workspace` SKIPS them (V-T5 verified via `--no-run`),
  `cargo xtask bench` passes `--features oxidizr-e2e/e2e-bench`; `1T` baseline
  saved + committed (60 files), `bench cmp 1T` verified. TESTING.md
  §Benchmarks + WORKFLOW.md table + `--help` updated.
- **Next:** **T6 close-out** per `build/PHASE_1T.md`: e2e-suite section in
  TESTING.md (quick/full tiers, corpus commands, coverage gate), PROGRESS
  board + log row, PHASE_1T all-DONE + no-tag note, root README dev-section
  mention, quartet + `cargo xtask ci lint` (actionlint for e2e.yml). **No tag**
  (user decision). Then **Phase 2 M0** (`build/PHASE_2.md`): `oxidizr-export`
  scaffold (deps core/json/diag — NO dsl/ast), `IndentingWriter`, `Diagram`
  types, CLI `export` skeleton (format dispatch + `structurizr-<wsId>-<viewKey>.
  <ext>` naming), theme.json de/ser in `oxidizr-json`, vendored default theme.
  M1 starts with EXPORT-PROPERTIES.md catalog + P2-V1..V6 oracle verifications.
- **Open questions:** T4 exit gate's *dispatched CI run* of `e2e.yml` needs a
  push — ask the user. Consider registering the 9001 Collector-wiring decision
  (diag owns CAP=100+note; pipeline unwired).
- **Don't re-derive (trust):** P1T-D1..D9 + T0–T5 landed blocks
  (`build/PHASE_1T.md`); `pitfalls/dsl-style-color-normalization.md` (+
  `group-url` verdict); `pitfalls/fmt-line-end-guards.md` (verbatim-magic-end);
  `pitfalls/t3-invalid-corpus-probes.md` (mutate replaces corpus/mutated
  wholesale — re-bless after); `pitfalls/cargo-bench-args.md` (never forward
  args to `cargo bench --workspace --`); NO_COLOR stderr has ANSI (strip before
  grep); fuzz mingw-CC-strip (`env/fuzz-host-toolchain.md`); oracle =
  `$OXIDZR_STRUCTURIZR_BIN` (`env/reference-oracle.md`, slow, one-shot
  `export --workspace F --format json --output DIR`).
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
