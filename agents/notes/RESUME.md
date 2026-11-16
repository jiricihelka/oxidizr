# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = Phase-1T planning commit (tip of `main`, tag
`v0.1.0` one commit down) · gates green · nothing pushed.

- **Done:** **PHASE 1 COMPLETE** (M0–M7, tag `v0.1.0` local — see PROGRESS log).
  **PHASE 1T PLANNED** (this commit): `build/PHASE_1T.md` — e2e regression +
  bench interlude. User decisions (2026-11-17, questions tool) recorded there:
  in-process stable API in a NEW `oxidizr-api` crate (CLI becomes argv/render
  shim — F9 registered), shared corpus `e2e/corpus/` (originals + committed
  mutations first, licensed public DSLs later), blessable manifests asserting
  exit class + OXZ code sets + structural counts (NEVER bytes/messages),
  ≥150 valid + ≥120 invalid floor, quick per-PR (rides `cargo test`) + full
  nightly `e2e.yml` env-gated (`OXIDZR_E2E_FULL=1`), benches local-only
  (`e2e-bench` cargo feature keeps CI's bench job light), no tag. PLAN.md +
  plan README + ADDITIONAL_FEATURES F9 + PROGRESS board updated.
- **Next:** Phase 1T **T0** on the user's go: V-T1 (engine purity for parallel
  in-process use) then extract `oxidizr-api` from `oxidizr-cli/src/lib.rs`
  (command bodies + `decode` + `json_loader`; suggestions stay CLI-side, D2).
  Gate: all existing tests unmodified (byte-proof). Then T1 harness crate +
  `xtask e2e bless`.
- **Open questions:** none (all settled via questions 2026-11-17).
- **Don't re-derive (trust + spot-check against `ad34036`):** the whole
  P1T-D1..D9 decision set in `build/PHASE_1T.md` (esp. why e2e is NOT in
  `testdata/`, why env-gate not `#[ignore]`, why `e2e-bench` feature); bench
  conventions + timings (PROGRESS P1-M7 log); fuzz + mingw CC strip (P1-M6d +
  `env/fuzz-host-toolchain.md`); reconcile/suggestions contracts (P1-M6c);
  `testing/cli-e2e-pitfalls.md` (UUID filter, EPIPE #6); JSON byte rules
  `WRITE-CATALOG.md`; env `agents/notes/env/`.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
