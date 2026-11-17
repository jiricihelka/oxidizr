# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = P2-M1-d commit (tip of `main`, tag `v0.1.0` far
down) · gates green · nothing pushed.

- **Done:** **PHASE 1 + 1T COMPLETE** (`v0.1.0`, T0–T6, no tag) · **P2-M0
  COMPLETE** (theme/json export byte-parity, see PROGRESS log) · **P2-M1-a/b**
  catalog `199bc70` + `core::styles` engine `95130bb` · **P2-M1-c** `export::
  themes::ThemeStore` + `paths` + OXZ-5008 `169e685` · **P2-M1-d** wiring
  (this commit): `ParseOptions.installed_themes` injection (FS-free dsl),
  `add_theme` installed branch (raw-token check, trim, no-dedup append),
  `export_cmd` loadThemes leg for `is_diagram()` formats BEFORE dispatch,
  CLI `--allow-net`/`--themes-dir`, D5 row, api `tests/export.rs` 4 legs.
  Oracle A/B: `themes` arrays byte-identical (witness `/tmp/nix-shell.uni100/
  p2/m1d`).
- **Next:** **P2-M1-e** load-path parity (found by the M1-d probe, details in
  `build/PHASE_2.md` M1-e): (1) port app-layer `Views.createDefaultViews`
  (reference `AbstractCommand` 109-112; witness keys `m1d/ref/workspace.json`
  — landscape order 1, context order 2, `generatedKey`) + call it at the
  shared load mirror + oracle A/B on a no-views workspace; (2) drop the
  trailing `""` line in `dsl_source_lines` (engine/mod.rs ~1218) so retained
  `structurizr.dsl` matches the reference's `readLine` accumulation (probe
  file must END with `\n` — vendored fixtures never do). THEN **M2**
  Structurizr-PlantUML exporter (`AbstractPlantUMLExporter` +
  `StructurizrPlantUMLExporter` port, `export_property` one-helper rule
  P2-D8, goldens + render smoke + oracle A/B byte compare).
- **Open questions:** T4 exit gate's *dispatched CI run* of `e2e.yml` needs a
  push — ask the user before any push (all P1/P1T/P2 commits sit unpushed).
  Consider registering the 9001 Collector-wiring decision. Vendored default
  theme still blocked (no DNS) — D3 interim rule (unknown name = keep working,
  unresolved styles) until the user supplies
  `static.structurizr.com/themes/default/theme.json`.
- **Don't re-derive (trust):** P2-D1..D11 + M1 decomposition + M1-e scope
  (`build/PHASE_2.md`, read fully before M2 code); `EXPORT-PROPERTIES.md` is
  THE property/theme-chain/constants contract; theme.json facts CODED + TESTED
  (`oxidizr-json/src/theme.rs`); P1T-D1..D9 (`build/PHASE_1T.md`);
  `testing/e2e-suite-operations.md`; oracle = `$OXIDZR_STRUCTURIZR_BIN`
  (`env/reference-oracle.md`, slow JVM — probe `export --workspace F --format
  <f> --output DIR`; witnesses `/tmp/nix-shell.uni100/p2/{v1,m0,m1d}`);
  `structurizr.dsl.identifier` UUIDs differ per CLI process (shape-assert,
  never bytes); NO NETWORK here; coverage gate FAILS if a covered code stays
  on the allow-list (5007/5008 entries remove when export manifests cover);
  `ApiOptions` now has 6 fields (`strict`/`keep_unknown`/`env`/`stdin`/
  `allow_net`/`themes_dir`) — every new literal must list all.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
