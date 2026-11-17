# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = M2-d part 2 fix-batch commit (tip of `main`, tag
`v0.1.0` far down) · gates green · nothing pushed.

- **Done:** **PHASE 1 + 1T COMPLETE** (`v0.1.0`, T0–T6, no tag) · **P2-M0**
  theme/json export byte-parity · **P2-M1 COMPLETE** (`199bc70`–`53d2e5e`) ·
  **P2-M2-a/b/c COMPLETE** (engine `oxidizr-export::plantuml` + `StructurizrDialect`,
  catalog `build/EXPORT-PLANTUML.md` §§1–14 THE contract) · **P2-M2-d part 1**
  dispatch leg (`ExportFormat::export` → `structurizr-<id>-<key>.puml`, viewless
  = NoteLine + exit 0) · **P2-M2-d part 2 fix batch (this commit)** — the full-corpus
  oracle A/B sweep (43 fixtures, 35 SAME) fully root-caused and fixed:
  (1) deployment-view ancestor chain (`viewops::view_add_deployment_element`
  kind-matched `DeploymentView.add` overloads; `groups` now byte-identical);
  (2) darker-stroke hex LOWER-CASE (`java_darker_hex` → `#{:06x}` — `setStroke`
  lowercases after `#%06X`); (3) dynamic-view RV order — `DynamicView.getRelationships()`
  override: stable re-sort by numeric `order` (all numeric) else by order-text,
  missing order first as `""` (never-crash; Java NPEs) — wired in BOTH the JSON
  writer (`view_content(…, dynamic)`) and the engine `view_relationships`
  (covers body/frames/sequence); (4) `custom` views store title/description
  VERBATIM incl. "" (`CustomView` ctor; JSON omits via NON_EMPTY, but
  `getName()` = `"Custom View: " + title` prints `Custom View: ` in puml;
  JSON-imported nulls keep `unwrap_or("null")`); (5) `source -/> dest { }`
  inheritance — `no_relationship` captures the FIRST removal's linked id
  mid-loop (Java reads `getLinkedRelationshipId()` off the still-live removed
  object; post-removal lookup is blind); (6) `model.customElements` = STORED
  order: DSL inserts at the FRONT (oracle witnesses: A..F export as F..A;
  Zulu,Alpha → Alpha,Zulu), JSON reader re-sorts numeric-id ascending
  (`setCustomElements` TreeSet rebuild), writer sorts nothing. All 6 target
  fixtures PUML-SAME vs archived oracle witnesses; JSON byte-equal after the
  established harness normalizations (inspection.* + random dsl.identifier
  GUIDs); `bulk-operations` stays DIFF = documented deviation **D13**
  (`!elements` fan-out identity-hash order — DEVIATIONS.md). Contracts
  recorded in WRITE-CATALOG §2/§5 (customElements, dynamic RV re-sort).
- **Next:** **M2-d part 3 — the proof assets**: plantuml goldens
  (`testdata/golden/plantuml/<fixture>/*.puml`, corpus = sweep SAME set minus
  deviation fixtures) via extended `xtask oracle gen|verify`, Category-A
  golden test (pattern: `oxidizr-dsl/tests/m5_goldens.rs`), render smoke
  (skip when plantuml absent), testdata/docs touches. Then M3 (C4-PlantUML
  dialect + `xtask export-verify` semantic comparator), M4 (Mermaid + `theme`
  export + CI render checks + benches), M5 (proptest panic suite, docs,
  tag `v0.2.0`).
- **Open questions:** T4 exit gate's *dispatched CI run* of `e2e.yml` needs a
  push — **USER DECLINED (2026-11-17): keep everything local**; the gate is
  documented push-pending, T0–T6 stay code-complete, do NOT re-ask. Vendored
  default theme still blocked (no DNS) — D3 interim rule (unknown name = keep
  working, unresolved styles) until the user supplies
  `static.structurizr.com/themes/default/theme.json`.
- **Don't re-derive (trust):** P2-D1..D11 + M1/M2 decomposition (`build/PHASE_2.md`);
  `build/EXPORT-PLANTUML.md` §§1–14 + WRITE-CATALOG §5 (updated 2026-11-17:
  customElements stored order, dynamic RV re-sort) are THE contracts. Oracle
  A/B corpus rules: exclude theme-URL fixtures (offline crash — `getting-started`,
  `exclude-relationships`, `test`), intentional-deviation fixtures (`script-*`
  D2, `text-block` D11 exit-1, `bulk-operations` D13); JSON A/B must strip the
  4 `structurizr.inspection.*` lines (+ dangling comma) and normalize random
  `structurizr.dsl.identifier` UUID chains (`<env>.<uuid>…`, random per parse
  BOTH sides — notes/testing/cli-e2e-pitfalls). Sweep witnesses:
  `/tmp/nix-shell.uni100/opencode/m2d/{sweep,jsweep,nr,bulkdet,custdet}/`.
  Oracle = `$OXIDZR_STRUCTURIZR_BIN` (slow JVM, minimal use); plantuml IS on
  PATH here. Pinned Java `/tmp/nix-shell.uni100/opencode/structurizr/`
  `ad34036` — the reference decides ambiguities; fix oxidizr, never
  expectations/goldens. `apply_text` stays for non-custom views (parsers call
  setters only on present tokens); `Ctx::NoRelInEnv.linked` now carries the
  captured static rel id (inheritance at `rel.rs` `relationship_parts`).
  `View::Dynamic` detection in the engine = `matches!` (no new ViewRef arms).
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
