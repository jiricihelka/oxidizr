# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = M2-a catalog commit (tip of `main`, tag `v0.1.0`
far down) · gates green · nothing pushed.

- **Done:** **PHASE 1 + 1T COMPLETE** (`v0.1.0`, T0–T6, no tag) · **P2-M0**
  theme/json export byte-parity · **P2-M1 COMPLETE** — a `199bc70` catalog,
  b `95130bb` `core::styles`, c `169e685` `ThemeStore`+`paths`, d `eadf24b`
  wiring, e-A `0759f96` retained-DSL `java_split_dsl_lines`, e-B `53d2e5e`
  default views (`create_default_views` + `apply_default_views` pipeline
  tail; m1d + m1e witnesses fully closed vs the reference). **P2-M2-a
  COMPLETE** — D12 `be26b56` (icon scale pinned `0.5`, zero-network),
  `export::property::export_property` `d179653` (the 2-level cascade,
  present-but-empty suppresses), catalog `build/EXPORT-PLANTUML.md` §§1–13
  (all micro-grammar line-exact: header order, element/relationship/boundary/
  group/legend formats, sequence path, style-footer 7 passes, idOf/filter,
  V5/V6 Structurizr halves, quirk register incl. mirror-the-bug findGroupStyle
  711).
- **Next:** **M2-b** engine port in `oxidizr-export::plantuml` — read
  `AbstractDiagramExporter` (848) FULLY first (per-view export methods,
  `writeElements` grouping incl. `getBoundarySoftwareSystems`/
  `getGroupableElements`, `getRelationshipsInView`, `typeOf`/terminology +
  metadata symbols, animation frames `frame` field + isVisible matrix —
  `EXPORT-PLANTUML.md` §4), append the engine-iteration section to the
  catalog. THEN M2-c (Structurizr dialect: PlantUMLStyle family + writers +
  legend), M2-d (dispatch + goldens + render smoke + oracle A/B).
- **Open questions:** T4 exit gate's *dispatched CI run* of `e2e.yml` needs a
  push — ask the user before any push (all P1/P1T/P2 commits sit unpushed).
  Consider registering the 9001 Collector-wiring decision. Vendored default
  theme still blocked (no DNS) — D3 interim rule (unknown name = keep working,
  unresolved styles) until the user supplies
  `static.structurizr.com/themes/default/theme.json`.
- **Don't re-derive (trust):** P2-D1..D11 + M1 decomposition + M1-e closure
  + M2 decomposition M2-a..d (`build/PHASE_2.md`, read fully before M2 code);
  `build/EXPORT-PLANTUML.md` is THE PlantUML grammar contract (header order,
  formats, sequence path, style footer, quirks — do not re-read the Java for
  line formats); `IndentingWriter` default indent = TWO spaces, `writeLine()`
  = bare newline, `toString` trims one trailing newline; P2-V3 settled (icon
  scale `0.5`, http-prefix gate — do not re-investigate);
  default views are APP-
  LAYER pipeline behavior (core = factory only, Locked list holds); Java name
  sorts = UTF-16 code units (`viewops::java_cmp_str` precedent); `ViewSet.
  isEmpty` = all nine collections, `Model.isEmpty` = people+systems+top-level
  nodes only; `EXPORT-PROPERTIES.md` is THE property/theme-chain/constants
  contract; P1T-D1..D9 (`build/PHASE_1T.md`); `testing/e2e-suite-operations.
  md`; oracle = `$OXIDZR_STRUCTURIZR_BIN` (`env/reference-oracle.md`, slow
  JVM — probe `export --workspace F --format <f> --output DIR`; witnesses
  `/tmp/nix-shell.uni100/p2/{v1,m0,m1d,m1e}`); `structurizr.dsl.identifier`
  UUIDs differ per CLI process (shape-assert, never bytes; A/B normalize
  `jq '(.. | objects | .["structurizr.dsl.identifier"]?) = "X"'` + strip
  `structurizr.inspection.*`); DSL view statements need KEY + DESCRIPTION
  before `{` (same-line brace); NO NETWORK here; coverage gate FAILS if a
  covered code stays on the allow-list; `ApiOptions` has 6 fields — every new
  literal must list all.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
