# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = M2-d part 3 closeout commit = **P2-M2 COMPLETE** (tip
of `main`, tag `v0.1.0` far down) · gates green · nothing pushed.

- **Done:** **PHASE 1 + 1T COMPLETE** (`v0.1.0`, T0–T6, no tag) · **P2-M0**
  theme/json export byte-parity · **P2-M1 COMPLETE** (`199bc70`–`53d2e5e`) ·
  **P2-M2 COMPLETE** (this commit): M2-a catalog `build/EXPORT-PLANTUML.md`
  §§1–14 + `export_property` + D12 · M2-b engine `oxidizr-export::plantuml` ·
  M2-c `StructurizrDialect` (`8d8e28f`) · M2-d part 1 dispatch (`033e482`),
  part 2 oracle A/B fix batch (`cc243a0`: ancestor chain, hex case, DynamicView
  RV re-sort, custom-view verbatim title, `-/>` linked capture, customElements
  STORED order; D13 registered), **part 3 proof assets (this commit)**: 30-fixture
  plantuml golden corpus `testdata/golden/plantuml/<fixture>/*.puml` (168 RAW
  oracle files = A/B sweep SAME set minus vacuous viewless/error-abort fixtures;
  byte-identical to the `/tmp/nix-shell.uni100/opencode/m2d/sweep/*/ref/`
  witnesses) via NEW `xtask oracle gen --plantuml` + plantuml leg in
  `oracle verify` (ran green end-to-end: 4 json + 30 plantuml sets);
  Category-A `oxidizr-api/tests/export_plantuml_goldens.rs` = byte-parity over
  the whole corpus (catch_unwind, reports all failures at once) + render smoke
  (`plantuml -tpng` over 4 representative goldens incl. legend; skips when
  plantuml absent — PR CI has none; renders here: plantuml 1.2026.9 IS on
  PATH). PROGRESS log + status board updated: M2 COMPLETE.
- **Next:** **M3** C4-PlantUML exporter (light/dark, stdlib property, sequence
  path) + `xtask export-verify` semantic comparator, oracle run green — the
  phase's reference-facing milestone (`build/PHASE_2.md`). Then M4 (Mermaid +
  `theme` export + CI render checks + export-corpus bench baseline), M5
  (proptest panic suite, docs, tag `v0.2.0`).
- **Open questions:** T4 exit gate's *dispatched CI run* of `e2e.yml` needs a
  push — **USER DECLINED (2026-11-17): keep everything local**; the gate is
  documented push-pending, T0–T6 stay code-complete, do NOT re-ask. Vendored
  default theme still blocked (no DNS) — D3 interim rule (unknown name = keep
  working, unresolved styles) until the user supplies
  `static.structurizr.com/themes/default/theme.json`.
- **Don't re-derive (trust):** P2-D1..D11 + M1/M2 decomposition (`build/PHASE_2.md`);
  `build/EXPORT-PLANTUML.md` §§1–14 + WRITE-CATALOG §5 (2026-11-17: stored-
  order customElements, dynamic RV re-sort) are THE contracts. Golden corpus
  rules (testdata/README.md): plantuml goldens are RAW (no normalization — the
  diagram writer injects no harness artifacts); JSON goldens strip the 4
  `structurizr.inspection.*` lines. Excluded fixtures by design: `script-*`
  (D2), `text-block` (D11 exit-1), `bulk-operations` (D13), theme-URL fixtures
  (offline oracle crash: `getting-started`, `exclude-relationships`, `test`);
  vacuous SAME (0 puml = viewless/abort) are ungoldenable, behaviorally covered
  in `export_plantuml.rs`. JSON A/B must also normalize random
  `structurizr.dsl.identifier` UUID chains (random per parse BOTH sides).
  Sweep witnesses: `/tmp/nix-shell.uni100/opencode/m2d/{sweep,jsweep,nr,bulkdet,custdet}/`
  (`ref/` = archived oracle output — NEVER re-run the oracle needlessly; slow
  JVM). Oracle = `$OXIDZR_STRUCTURIZR_BIN` (2026.10.26). Pinned Java
  `/tmp/nix-shell.uni100/opencode/structurizr/` `ad34036` — the reference
  decides ambiguities; fix oxidizr, never expectations/goldens. M3 heads-up:
  C4-PlantUML needs the stdlib (`!include <C4/C4_Container>`) — plantuml jar on
  PATH bundles stdlib offline, but the EXPORTED diagram from the reference
  references C4 includes; the semantic comparator (M3 spec) must tolerate
  layout/styling freedom per `build/PHASE_2.md` M3 scope.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
