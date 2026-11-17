# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = M3-b closeout commit = **P2-M3-b DONE** (tip of
`main`, tag `v0.1.0` far down) · gates green · nothing pushed.

- **Done:** **PHASE 1 + 1T COMPLETE** (`v0.1.0`, T0–T6, no tag) · **P2-M0**
  theme/json export byte-parity · **P2-M1 COMPLETE** (`199bc70`–`53d2e5e`) ·
  **P2-M2 COMPLETE** (`ab5a07b`: 30-fixture plantuml golden corpus + oracle
  verify green + render smoke) · **M3-a** (`43a8f79`): M3-a..d decomposition in
  `build/PHASE_2.md` + catalog `build/EXPORT-C4-PLANTUML.md` + C4 oracle witness
  sweep of the 43-fixture corpus (`/tmp/nix-shell.uni100/opencode/m2d/sweep/*/
  ref-c4/` + `ref-c4.status`; the 10 exit-1 dirs are vacuous) ·
  **M3-b (this commit):** `C4PlantUmlDialect` (`oxidizr-export::plantuml::c4`) —
  full header port (ALWAYS `<style>` root block via `defaults`, direction line,
  skinparams, `<C4/…>` includes, stdlib property, `c4plantuml.tags` section in
  Java `HashMap` order via `oxidizr_json::javahash::entry_order`), boundary/
  group/element/Rel macros with the quirk register (`, $borderThickness="1")`
  drop, boundary `#ffffff`-bg + stroke-as-font, `"<id>_boundary"` aliases,
  `group_<n>` counter, `(xN)` instance suffixes, response endpoint swap,
  `WithoutPropertyHeader`+`AddProperty` legs), footer `SHOW_LEGEND`/stereotypes;
  engine hook `Dialect::exports_custom_views` (C4 → custom views write NO
  file); `format.rs` C4 arm; shared structurizr helpers `pub(crate)`;
  `oxidizr-json` `javahash` now `pub`; API 5007 placeholder flipped to
  behavior tests (diagram-only files, no `-key` legends, custom-view skip);
  format-level unit tests incl. `-dark` defaults.
- **Next:** **M3-c** — byte A/B sweep vs the `ref-c4/` witnesses (script the
  33 witnessed fixtures; root-cause every DIFF against pinned Java
  `C4PlantUMLExporter.java` `ad34036`, fix oxidizr only) · **M3-d** — c4 goldens
  `testdata/golden/plantuml-c4/` (extend `xtask oracle gen --plantuml` with
  `--c4`; drop the `refs:ignore` line in `PHASE_2.md` when the dir exists),
  golden test extension, render smoke (plantuml 1.2026.9 renders `<C4/…>`
  offline), semantic comparator `xtask/src/exportcmp/` + `cargo xtask
  export-verify` (Category B), ONE sparing oracle run green. Then M4 (Mermaid +
  CI render checks + export-corpus bench baseline), M5 (proptest panic suite,
  docs, tag `v0.2.0`) per `build/PHASE_2.md`.
- **Open questions:** T4 exit gate's *dispatched CI run* of `e2e.yml` needs a
  push — **USER DECLINED (2026-11-17): keep everything local**; the gate is
  documented push-pending, T0–T6 stay code-complete, do NOT re-ask. Vendored
  default theme still blocked (no DNS) — D3 interim rule (unknown name = keep
  working, unresolved styles) until the user supplies
  `static.structurizr.com/themes/default/theme.json`.
- **Don't re-derive (trust):** P2-D1..D13 + M1–M3 decomposition
  (`build/PHASE_2.md`); `build/EXPORT-C4-PLANTUML.md` is THE C4 contract (the
  port mirrors the file, not memory); `build/EXPORT-PLANTUML.md` §§1–14 stays
  THE base-engine contract. C4 specifics: witness dirs are `/tmp/.../m2d/sweep/
  */ref-c4/` (NEVER re-run the oracle needlessly — slow JVM); C4 property
  defaults legend=true / stereotypes=false / tags=false / stdlib=true /
  elementProperties=false / relationshipProperties=false (all `equalsIgnoreCase
  ("true")`; `plantuml.title` stays case-SENSITIVE); `id_of` is HIERARCHICAL
  (matches Java `idOf` — boundary/DN/element macros all use it); entry maps
  emulate Java `HashMap.put` (dup key replaces IN PLACE, iteration =
  `entry_order`); D12 icon pin (`{scale=0.5}`) applies to C4 sprites too; NO
  vendored fixture sets a `c4plantuml.*` or `plantuml.sequenceDiagram`
  property — those legs get fresh probes in M3-d. Golden corpus rules +
  fixture exclusions: unchanged from M2-d (testdata/README.md). Oracle =
  `$OXIDZR_STRUCTURIZR_BIN` (2026.10.26); plantuml 1.2026.9 on PATH; NO DNS.
  Pinned Java `/tmp/nix-shell.uni100/opencode/structurizr/` `ad34036` — the
  reference decides ambiguities; fix oxidizr, never expectations/goldens.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
