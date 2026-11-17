# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = P2-M0 docs commit (tip of `main`, tag `v0.1.0` far
down) · gates green · nothing pushed.

- **Done:** **PHASE 1 + 1T COMPLETE** (`v0.1.0`, T0–T6, no tag) · **PHASE 2 M0
  COMPLETE**: P2-V1 VERIFIED (theme contract incl. style doc-order + setter
  clamps — WRITE-CATALOG §5/§6, commit 6f6cc64); `oxidizr-export` crate
  (IndentingWriter/Diagram beans/format registry/naming, 11 tests);
  `oxidizr_json::theme` de/ser byte-exact vs oracle witnesses (`theme_json.rs`);
  api `Request::Export`+`export_cmd` mirroring `ExportCommand` (mkdirs before
  dispatch, unknown format = note + exit 0, OXZ-5007 unimplemented); cli
  `export -w/-f/-o`; A/B: `-f theme` byte-identical to out1/out6 (commit
  ca292e7). Evidence: `build/PHASE_2.md` (P2-D11 V1 checkbox) + PROGRESS log.
- **Next:** **PHASE 2 M1** per `build/PHASE_2.md`: FIRST the
  `EXPORT-PROPERTIES.md` catalog (P2-D8 — exporter property reads, from the
  pinned monorepo, do not port from memory) + P2-V2/V3/V4 verifications, THEN
  `core::styles::resolve` (P2-D2) + `ThemeStore` in `oxidizr-export` (P2-D3:
  `--themes-dir`/`OXIDZR_THEMES_DIR`, cache, `--allow-net` — the flag ships
  here, D5 register update, error UX), wire `loadThemes` into the diagram
  branch of `export_cmd`. Default-theme vendoring DEFERRED (no DNS to
  static.structurizr.com; file absent from the pinned monorepo) — record if
  M2 needs it. M1 is >4 modules: decompose in PHASE_2.md before code (charter).
- **Open questions:** T4 exit gate's *dispatched CI run* of `e2e.yml` needs a
  push — ask the user before any push (all P1/P1T/P2 commits sit unpushed).
  Consider registering the 9001 Collector-wiring decision.
- **Don't re-derive (trust):** P2-D1..D11 + V1 evidence + deferral
  (`build/PHASE_2.md`, read fully before M1 code); theme.json facts are CODED
  + TESTED (`oxidizr-json/src/theme.rs`); P1T-D1..D9 (`build/PHASE_1T.md`);
  `testing/e2e-suite-operations.md`; oracle = `$OXIDZR_STRUCTURIZR_BIN`
  (`env/reference-oracle.md`, slow JVM — probe pattern `export --workspace F
  --format <f> --output DIR`; witnesses kept `/tmp/nix-shell.uni100/p2/v1`
  + `p2/m0`); `structurizr.dsl.identifier` UUIDs differ per CLI process
  (shape-assert, never bytes); NO NETWORK on this machine; coverage gate
  FAILS if a covered code stays on the allow-list (drop the 5007 entry when
  export manifests cover it).
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
