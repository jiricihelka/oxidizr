# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = P2-M1-b commit (tip of `main`, tag `v0.1.0` far
down) · gates green · nothing pushed.

- **Done:** **PHASE 1 + 1T COMPLETE** (`v0.1.0`, T0–T6, no tag) · **P2-M0
  COMPLETE** (theme/json export byte-parity, see PROGRESS log) · **P2-M1-a**
  `EXPORT-PROPERTIES.md` catalog + V2(matrix+props)/V4 VERIFIED + M1 decomposed
  (`199bc70`) · **P2-M1-b** `oxidizr-core::styles` resolution engine (P2-D2:
  find_*/element_style/relationship_style, `Styles.themes`, copyFrom,
  Color.decode/darker; 12 tests, `95130bb`).
- **Next:** **P2-M1-c** `oxidizr-export::themes::ThemeStore` + `paths` module
  (XDG/APPDATA cache + hand-rolled SHA-256, installed-name registry per
  `EXPORT-PROPERTIES.md` §6.2: `--themes-dir` > `OXIDZR_THEMES_DIR` >
  `<ws-parent>/themes` > `STRUCTURIZR_THEMES`, one-level `theme.json` scan;
  URL branch behind `--allow-net` via curl delegate + content-type check +
  relative-icon rewrite; vendored default theme stays blocked (no DNS) — D3
  interim rule), then **M1-d** wiring: registry BEFORE DSL parse (parser
  `add_theme` gains the installed-name branch), `loadThemes` mirror in the
  diagram branch of `export_cmd`, CLI flags + `ApiOptions`, D5 register row,
  oracle A/B probe with a local themes dir. Then M2 (Structurizr-PlantUML).
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
