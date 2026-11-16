# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-16 · HEAD = M5 wrap-up commit (tip of `master`) · gates green ·
nothing pushed.

- **Done:** P1-M5 `workspace extends` COMPLETE (M5-a `WorkspaceBuilder::adopt`
  9f174e6; M5-b engine `extend_from` + JSON-loader hook + `configureFrom` +
  `extending_workspace` reuse; M5-c OXZ-5004 tombstone, burn-in rows retired,
  `m5_extends.rs` 7 tests, `extend/4.dsl` golden byte-exact, `xtask oracle`
  fixes + `testing/oracle-app-harness.md` note — all in PROGRESS.md M5 log);
  P1-M4 (engine + burn-in, host + Wine); infra/tooling through 2576a25
  (`refs` 4th gate, CI lint job, bench save/cmp, dev-shell tools, ast-grep,
  execution-model docs).
- **Next:** GOAL = finish Phase 1 → **M6** (decomposition already in
  PHASE_1.md §P1-D11): M6-a CLI surface (commands, flags, exit codes, miette
  rendering + suggestions, json2json with `--keep-unknown`, inject
  `oxidizr_json` as the engine's `json_loader`), M6-b formatter on the
  lossless CST (+ idempotency/verbatim invariants, proptest), M6-c reconcile
  + validate polish, M6-d fuzz targets + CI nightlies. Then M7 (corpus
  parse-perf < 100 ms check, README/docs polish, v0.1.0 tag).
- **Open questions:** none. (M6-a decides: file encoding handling V-8 —
  reference `parse(File)` reads UTF-8 by default, `setCharacterEncoding`
  exists; CLI flag or UTF-8-only at v0.1?)
- **Don't re-derive (trust + spot-check against `ad34036`):** M4 findings in
  `agents/plan/PROGRESS.md` errata + `agents/plan/DSL-ERRATA.md`; M5 findings
  in PROGRESS M5 log + `testing/oracle-app-harness.md` (export injects
  inspection props + default views — goldening limits); JSON byte rules in
  `agents/plan/build/WRITE-CATALOG.md`; env facts in `agents/notes/env/`;
  critcmp takes only in-root baseline names (`bench cmp` stages, see main.rs doc).
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
