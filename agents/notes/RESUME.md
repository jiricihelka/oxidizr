# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-18 · HEAD = **P2-M4-b/M4-c commit** (tip of `main`, tag
`v0.1.0` far down) · gates green · nothing pushed.

- **Done:** **PHASE 1 + 1T COMPLETE** (`v0.1.0`, no push — user declined) ·
  **P2-M0** · **P2-M1** · **P2-M2** (`ab5a07b`) · **P2-M3 COMPLETE** (M3-a
  `43a8f79`, M3-b `2706209`, M3-c sweep 30 SAME/4 design-DIFF/10 vacuous,
  M3-d `06e14ae`: c4 goldens 30 fixtures + oracle verify legs + `exportcmp`
  comparator + `export-verify`) · **M4-a** (`fee6e64`: catalog
  `build/EXPORT-MERMAID.md` + M4-a..d decomposition + mermaid witness sweep
  33/44 fixtures) · **M4-b + M4-c (this commit)**: `oxidizr-export::mermaid`
  (`MermaidDialect` full port of the catalog §§1–§9) + engine deltas
  (`Dialect::owns_sequence_document`/`sequence_document` route — Mermaid's
  sequence form is a STANDALONE document, NOT the PlantUML walk; `mmd` ext
  via `create_diagram`) + `ExportFormat::Mermaid` dispatch (Light scheme) +
  10 unit + 4 API + 1 format test + the stale OXZ-5007 doc notes updated
  (only `websequencediagrams`/`static` stay unlanded). **M4-c byte sweep**
  vs the `ref-mermaid/` witnesses: **30 SAME / 3 DIFF / 11 vacuous** —
  DIFFs = `bulk-operations` (D13 rel-id fan-out, catalog §11) + the two D2
  script fixtures (loader exit 1); zero port bugs; script
  `/tmp/nix-shell.uni100/opencode/m4/sweep-ab.sh`, result
  `m4/diff-list.txt`.
- **Next:** **M4-d** per `build/PHASE_2.md`: (1) `xtask oracle gen/verify
  --mermaid` legs (`crates/xtask/src/main.rs` — mirror the `--c4` flags:
  `reference_export_plantuml(oracle, ws, outdir, "mermaid")` already
  parameterized; the verify walk needs the `.mmd` extension); (2)
  `testdata/golden/mermaid/` = the 30 SAME fixtures (bulk-operations OUT,
  scripts OUT) + `testdata/golden/theme/` goldens + Category-A parity tests
  (extend `export_plantuml_goldens.rs` or new `export_mermaid_goldens.rs`);
  remove the `refs:ignore` markers on the M4-d bullet in `PHASE_2.md` when
  the dirs exist; (3) CI render checks (loud skip; `mmdc` ABSENT here,
  plantuml 1.2026.9 present); (4) `cargo xtask bench` export corpus +
  `bench save M4`. Then **M5**: proptest panic suite, docs errata fold-back
  (`oxidizr export --help`), tag `v0.2.0` (LOCAL, never push). Vendored
  default theme still blocked (no DNS) — D3 interim rule.
- **Open questions:** none. Phase 1T dispatch gate **USER DECLINED push
  (2026-11-17): keep everything local** — do NOT re-ask.
- **Don't re-derive (trust):** P2-D1..D13 + decomposition (`build/PHASE_2.md`);
  `build/EXPORT-MERMAID.md` is THE Mermaid contract (the §1 format table was
  byte-confirmed by the 30 SAME witnesses — incl. rel line `1-. "…" .->2`,
  element divs `80%; margin-top:10px` / `70%; margin-top: 0px`, DN boundary
  stroke+color vs system/container stroke-as-font, group cut `#cccccc`
  fallback, `findElementStyle(tag)` None-when-undeclared chain);
  `build/EXPORT-C4-PLANTUML.md` THE C4 contract. Core gotchas: `Styles`
  fields are `element_styles`/`relationship_styles`/`themes`; merged
  relationship_style base is `style=Dashed, dashed=true` (solid ⇔
  `style==Some(Solid) || dashed==Some(false)`). Oracle verify cost ~40 s/
  fixture-set — run sparingly; witnesses at `/tmp/nix-shell.uni100/opencode/
  m2d/sweep/*/ref-mermaid/` (+`ref-c4/`, `ref-plantuml/`); NEVER re-run the
  oracle needlessly. Golden corpus rules: `testdata/README.md`. Oracle =
  `$OXIDZR_STRUCTURIZR_BIN` (2026.10.26); plantuml 1.2026.9 on PATH (C4
  stdlib bundled, offline); **mmdc ABSENT**; NO DNS. Pinned Java
  `/tmp/nix-shell.uni100/opencode/structurizr/` `ad34036` — the reference
  decides ambiguities; fix oxidizr, never expectations/goldens.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
