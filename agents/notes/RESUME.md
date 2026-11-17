# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = M3-d closeout commit = **P2-M3 COMPLETE** (tip of
`main`, tag `v0.1.0` far down) · gates green · nothing pushed.

- **Done:** **PHASE 1 + 1T COMPLETE** (`v0.1.0`, T0–T6, no tag) · **P2-M0** ·
  **P2-M1 COMPLETE** · **P2-M2 COMPLETE** (`ab5a07b`) · **P2-M3 COMPLETE** —
  M3-a (`43a8f79`: catalog `build/EXPORT-C4-PLANTUML.md` + 44-fixture oracle
  witness sweep), M3-b (`2706209`: `C4PlantUmlDialect` port +
  `exports_custom_views` hook + dispatch), M3-c (A/B sweep: 30 SAME / 4
  design-DIFF (`script-inline`, `script-in-dynamic-view` D2; `text-block` D11;
  `bulk-operations` D13) / 10 vacuous — script
  `/tmp/nix-shell.uni100/opencode/m3c/sweep.sh`, witnesses + our output under
  the M2-d sweep dirs), **M3-d (this commit)**: `xtask oracle gen --plantuml
  --c4` + c4 verify leg (oracle verify green 2026-11-17: 4 json + 59 plantuml
  sets, 0 mismatches), 30-fixture `testdata/golden/plantuml-c4/` corpus (29 +
  the new `probes/c4-props.dsl` covering `c4plantuml.tags/.stereotypes/
  .elementProperties/.relationshipProperties/.stdlib=false` +
  `plantuml.sequenceDiagram` — byte-identical), Category-A
  `c4plantuml_goldens_byte_parity` + offline C4 render smoke (both corpora in
  `export_plantuml_goldens.rs`), and the P2-D9 semantic comparator
  (`xtask/src/exportcmp/` + `cargo xtask export-verify`, default corpus = c4
  goldens + `bulk-operations` which it proves SEMANTICALLY EQUAL; needs
  `$OXIDZR_STRUCTURIZR_BIN` + `target/debug/oxidizr`).
- **Next:** **M4** per `build/PHASE_2.md` — Mermaid exporter (`ExportFormat::
  Mermaid` → OXZ-5007 today) + `theme` export polish (M0 dispatch exists;
  re-export a workspace theme), render checks wired in CI (skip loudly when
  tools absent; `$OXIDZR_PLANTUML_BIN`/`$OXIDZR_MERMAID_BIN` rule in
  `build/PHASE_2.md` §P2-D9), export-corpus bench baseline
  (`cargo xtask bench save M4`). Then **M5**: proptest panic suite for
  exporters, docs (`oxidizr export --help` + errata fold-back), tag `v0.2.0`
  (local). Vendored default theme still blocked (no DNS) — D3 interim rule
  until the user supplies `static.structurizr.com/themes/default/theme.json`.
- **Open questions:** Phase 1T dispatch gate **USER DECLINED push (2026-11-17):
  keep everything local** — do NOT re-ask. Nothing else open for M4.
- **Don't re-derive (trust):** P2-D1..D13 + decomposition (`build/PHASE_2.md`);
  `build/EXPORT-C4-PLANTUML.md` is THE C4 contract; `build/EXPORT-PLANTUML.md`
  §§1–14 the base engine. Comparator whitelist = comment/blank lines only;
  `exportcmp::C4_MACROS` = the P4 importer's macro inventory (reference-pinned:
  no `Enterprise_Boundary`/`*_Ext`/`Rel_Neighbor` in this exporter version —
  element macros are Person/System/Container[Db|Queue]/Component[Db|Queue]/
  Deployment_Node, boundaries Boundary/System_Boundary/Container_Boundary).
  Oracle verify cost: ~40 s/fixture-set — run sparingly; witnesses at
  `/tmp/.../m2d/sweep/*/ref-c4/` (NEVER re-run the oracle needlessly).
  `custom-view-animation` is C4-VACUOUS (0 files, oracle exit 0) — keep it out
  of c4 goldens. Golden corpus rules: testdata/README.md. Oracle =
  `$OXIDZR_STRUCTURIZR_BIN` (2026.10.26); plantuml 1.2026.9 on PATH (C4 stdlib
  bundled, offline); NO DNS. Pinned Java `/tmp/nix-shell.uni100/opencode/
  structurizr/` `ad34036` — the reference decides ambiguities; fix oxidizr,
  never expectations/goldens.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
