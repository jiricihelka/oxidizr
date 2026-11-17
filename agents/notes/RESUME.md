# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = T3 commit (tip of `main`, tag `v0.1.0` far down) ·
gates green · nothing pushed.

- **Done:** **PHASE 1 COMPLETE** (M0–M7, tag `v0.1.0` local). **PHASE 1T T0–T3
  DONE**: T0 `oxidizr-api` extraction; T1 harness + seed (`d8d19f5`); T2 valid
  corpus 156 (`cfd75eb`); T3 (this commit) = invalid corpus **24 authored + 120
  mutated = 144** + `cargo xtask e2e mutate [SEED]` (splitmix64, 5 ops, every
  mutant verified-failing at generation; V-T3: same seed ⇒ byte-identical) +
  `reconcile` manifest section (entries with BOTH inputs — 3002 enters via
  `invalid/09`) + coverage gate **11/14 codes**, allow-list shrunk 7→3:
  {1001 bug-compat, 4002 hydrate-gated, 9001 collector-unwired}. Quartet +
  fuzz-check green. Notes: `pitfalls/t3-invalid-corpus-probes.md` (READ before
  touching the invalid corpus or the allow-list).
- **Next:** **T4** per `build/PHASE_1T.md` — D6 invariants (parse determinism,
  dsl2json→json2json byte-stability, parse∘fmt ≡ parse, self-reconcile) behind
  env `OXIDZR_E2E_FULL=1` (NOT `#[ignore]`), nightly `.github/workflows/e2e.yml`
  (dispatch + schedule, exports the env, runs `cargo test --workspace`), V-T4 =
  inventory today's `#[ignore]`d tests. Then T5 (`e2e_` benches behind feature
  `e2e-bench` + `cargo xtask bench save 1T` + docs), T6 close-out (TESTING.md
  section, PROGRESS board+log, PHASE_1T DONE rows, root README dev-section;
  **no tag**). Then Phase 2 (`build/PHASE_2.md`).
- **Open questions:** none. Flag for oracle check: engine rejects `url` inside
  `group` though vendored `testdata/dsl/group-url.dsl` exists — Phase-1 compat
  gap candidate, not yet in DEVIATIONS.md. Also consider registering the 9001
  Collector-wiring decision (diag crate owns CAP=100+note; pipeline ignores it).
- **Don't re-derive (trust):** P1T-D1..D9 + T0–T3 landed blocks
  (`build/PHASE_1T.md`); `pitfalls/t3-invalid-corpus-probes.md` (3001 =
  person-vs-system name clash; 4002 gated at read; 9001 unwired; `theme <url>`
  parses CLEAN; `!plugin`→5002, `!adrs <existing-dir>`→5003; mutate replaces
  corpus/mutated wholesale — re-bless after);
  `pitfalls/dsl-corpus-grammar-gotchas.md` (T2 grammar traps);
  `pitfalls/reconcile-ambiguity-fixtures.md`; NO_COLOR stderr still has ANSI
  (strip before grep); fuzz mingw-CC-strip (`env/fuzz-host-toolchain.md`);
  JSON byte rules `WRITE-CATALOG.md`.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
