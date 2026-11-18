# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-18 · HEAD = **P2-M5 commit** (tip of `main`) · tag **`v0.2.0`
(local) = Phase 2 COMPLETE** · gates green · nothing pushed.

- **Done:** **PHASE 1 + 1T COMPLETE** (`v0.1.0`, no push — user declined) ·
  **PHASE 2 COMPLETE** (M0–M5, `v0.2.0`): M0 scaffold+theme/json · M1 theme
  infra (`199bc70`–`53d2e5e`) · M2 Structurizr-PlantUML · M3 C4-PlantUML
  (`06e14ae` goldens+comparator) · M4 Mermaid: `fee6e64` catalog, `506040e`
  `mermaid.rs` port + dispatch (sweep 30 SAME / 3 known-DIFF), `9b367d6`
  goldens (`testdata/golden/mermaid/` 30 fixtures × 84 `.mmd`,
  `golden/theme/` 6 fixtures) + xtask `--mermaid`/`--theme` legs +
  `export_mermaid_goldens.rs` + `bench_export` · **M5 (this commit)**:
  proptest panic suite `crates/oxidizr-export/tests/export_never_panics.rs`
  (256 valid random workspaces × every landed format, naming invariants),
  docs refresh (CLI `export --help` notes, README status+quickstart,
  `docs/diagnostics.md` OXZ-5007 row), P2-V5/V6 verification items closed
  with evidence, full `cargo xtask oracle verify` green — **4 json + 90
  diagram + 6 theme sets match** — bench baseline `7f4b428`
  (`bench-baseline/M4/`, 17 dirs).
- **Next:** Phase 3 (viewer + layout, `build/PHASE_3.md` when written — see
  `agents/plan/README.md` roadmap). Standing leftover inside P2: the
  **vendored default theme** (`static.structurizr.com/themes/default/
  theme.json`) needs the user to supply the file — no DNS here; D3 interim
  rule in force (`build/PHASE_2.md` M1 amendment documents it).
- **Open questions:** none. Phase 1T dispatch gate **USER DECLINED push
  (2026-11-17): keep everything local** — do NOT re-ask. Tags `v0.1.0`,
  `v0.2.0` are LOCAL only.
- **Don't re-derive (trust):** P2-D1..D13 + all decomposition/verification
  records (`build/PHASE_2.md`, checkboxes closed); THE contracts:
  `build/EXPORT-PROPERTIES.md`, `build/EXPORT-PLANTUML.md`,
  `build/EXPORT-C4-PLANTUML.md`, `build/EXPORT-MERMAID.md` (the ports mirror
  the files, not memory; §11 = corpus exclusions). Core gotchas: `Styles`
  fields `element_styles`/`relationship_styles`/`themes`; merged
  relationship_style base is `style=Dashed, dashed=true`. Oracle verify
  summary: "N json + N diagram + N theme golden set(s) match"; witnesses at
  `/tmp/nix-shell.uni100/opencode/m2d/sweep/*/ref-{mermaid,c4,plantuml}/`;
  30-fixture mermaid corpus list `/tmp/nix-shell.uni100/opencode/m4/
  same-list.txt`; NEVER re-run the oracle needlessly (~40 s/fixture-set;
  full verify ≈ all 100 sets, only pre-tag/weekly CI). Oracle =
  `$OXIDZR_STRUCTURIZR_BIN` (2026.10.26); plantuml 1.2026.9 on PATH; **mmdc
  ABSENT** (render smokes LOUD-SKIP); NO DNS. Pinned Java
  `/tmp/nix-shell.uni100/opencode/structurizr/` `ad34036` — the reference
  decides; fix oxidizr, never expectations/goldens. Clippy caps: >3 bools →
  enum/bitmask, fn >100 lines → extract, `doc_markdown` backticks.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
