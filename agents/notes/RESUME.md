# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = T4 commits (tip of `main`, tag `v0.1.0` far down) ·
gates green · nothing pushed.

- **Done:** **PHASE 1 COMPLETE** (M0–M7, tag `v0.1.0` local). **PHASE 1T T0–T4
  DONE**: T0 `oxidizr-api`; T1 harness+seed; T2 valid corpus 156; T3 invalid
  corpus 144 + `xtask e2e mutate`; T4 (this commit) = **full-tier invariants**
  `crates/oxidizr-e2e/tests/full.rs` (parse determinism, json round-trip
  byte-stability, fmt fixpoint+semantic via scratch-dir replay, self-reconcile
  identity) behind `OXIDZR_E2E_FULL=1` + nightly `.github/workflows/e2e.yml`.
  **The net caught 2 real engine bugs, both fixed with oracle verdicts:**
  (1) style colors normalize on the DSL path too — `Color` port now
  `oxidizr-core::colors`, DSL `background/colour/stroke` run it (bogus → 2001
  statement error); (2) `fmt` prints a logical line VERBATIM when its joined
  value ends `"""` already in the source (magic hidden behind `\`-join) —
  proptest seed `0c3752…` pinned. V-T4 closed (zero `#[ignore]`d tests).
  Also closed: `group { url }` IS rejected by the reference — vendored
  `testdata/dsl/group-url.dsl` is a negative fixture, our message matches
  byte-identically (no DEVIATIONS entry). Notes:
  `pitfalls/dsl-style-color-normalization.md`, updated
  `pitfalls/fmt-line-end-guards.md`. Quartet green.
- **Next:** **T5** per `build/PHASE_1T.md` — `e2e_` criterion benches behind
  feature `e2e-bench` in `crates/oxidizr-e2e` (over the corpus: dsl2json
  parse-only, json2json, full `execute` pipeline, fmt), `cargo xtask bench
  save 1T` baseline, xtask bench passes `--features e2e-bench`, TESTING.md
  §Benchmarks paragraph + WORKFLOW.md command-table + `cargo xtask --help`
  line; **V-T5**: CI bench job (`cargo bench --workspace`, NO feature) still
  green+fast (feature-gated benches listed-but-skipped). Then **T6** close-out
  (TESTING.md section, PROGRESS board+log, root README dev-section, **no tag**).
  Then **Phase 2** (`build/PHASE_2.md`).
- **Open questions:** T4 exit gate includes a *dispatched CI run* of `e2e.yml`
  — requires a push; ask the user before pushing. Consider registering the
  9001 Collector-wiring decision (diag crate owns CAP=100+note; pipeline
  ignores it).
- **Don't re-derive (trust):** P1T-D1..D9 + T0–T4 landed blocks
  (`build/PHASE_1T.md`); `pitfalls/dsl-style-color-normalization.md` (+ its
  `group-url` verdict); `pitfalls/fmt-line-end-guards.md` (verbatim-magic-end
  rule); `pitfalls/t3-invalid-corpus-probes.md` (mutate replaces corpus/mutated
  wholesale — re-bless after); `pitfalls/dsl-corpus-grammar-gotchas.md`;
  `pitfalls/reconcile-ambiguity-fixtures.md`; NO_COLOR stderr still has ANSI
  (strip before grep); fuzz mingw-CC-strip (`env/fuzz-host-toolchain.md`);
  oracle = `$OXIDZR_STRUCTURIZR_BIN` (`env/reference-oracle.md`, slow, one-shot
  `export --workspace F --format json --output DIR`).
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
