# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-16 · HEAD `2576a25` · gates green · nothing pushed.

- **Done:** P1-M4 (engine + burn-in, green on host AND Wine `ci windows`);
  post-M4 infra (Wine leg, CI caches, bench-job fix, `cargo xtask ci|bench|cov`,
  `agents/` reorg, charter slimming); workflow tooling: dev-shell triage tools
  (jq/sqlite/python3/actionlint/critcmp + ast-grep w/ search recipes note),
  `cargo xtask refs` = 4th default gate  (quartet fmt+clippy+test+refs), `ci lint` step + CI `lint` job (actionlint
  1.7.12 pinned+sha256), `bench save <LABEL>` / `bench cmp <LABEL>` (critcmp
  staging verified vs M0), `wineserver -k` after the windows leg.
- **Next:** GOAL = finish Phase 1. **M5 = `workspace extends` only** (everything
  else landed with M4 — see PHASE_1.md §P1-D11 reality note). M5-a DONE:
  `WorkspaceBuilder::adopt` (core). M5-b: engine extends arm per
  `WorkspaceParser.java` — base REPLACES the workspace; `.json` via injected
  loader hook (dsl must not depend on json; CLI injects, dsl tests dev-dep);
  `.dsl` recursive child parse + adopt + `configureFrom` (idents scope,
  archetypes, consts/vars); JSON bases re-register `structurizr.dsl.identifier`;
  clearDsl + not-portable. M5-c: retire the 11 OXZ-5004 `KNOWN_ERRORS` rows
  (URLs → OXZ-5001), `m5_extends.rs` vs `DslTests`, oracle goldens for
  `testdata/dsl/extend/`, full-fixture gate. Then M6 (decomposed in PHASE_1:
  CLI/fmt/reconcile/fuzz) and M7 (perf, docs, v0.1.0).
- **Open questions:** none.
- **Don't re-derive (trust + spot-check against `ad34036`):** M4 findings in
  `agents/plan/PROGRESS.md` errata + `agents/plan/DSL-ERRATA.md`; JSON byte rules
  in `agents/plan/build/WRITE-CATALOG.md`; env facts in `agents/notes/env/`;
  critcmp takes only in-root baseline names (`bench cmp` stages, see main.rs doc).
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
