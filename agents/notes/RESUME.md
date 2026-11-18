# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-18 · HEAD = **P2-M4-d commit** (tip of `main`, tag `v0.1.0`
far down) · gates green · nothing pushed · **M4 COMPLETE → M5 remains**.

- **Done:** **PHASE 1 + 1T COMPLETE** (`v0.1.0`, no push — user declined) ·
  **P2-M0** · **P2-M1** · **P2-M2** (`ab5a07b`) · **P2-M3 COMPLETE** (M3-d
  `06e14ae`) · **M4-a** (`fee6e64`: catalog `build/EXPORT-MERMAID.md` +
  witness sweep) · **M4-b/c** (`506040e`: `mermaid.rs` port + engine
  `owns_sequence_document`/`sequence_document` hooks + dispatch; byte sweep
  30 SAME / 3 known-DIFF / 11 vacuous, zero port bugs) · **M4-d (this
  commit)**: `testdata/golden/mermaid/` (30 fixtures × 84 `.mmd`, copied
  byte-exact from the `ref-mermaid/` witnesses — no needless oracle re-run)
  + `testdata/golden/theme/` (6 fixtures, oracle-generated live, all SAME vs
  our binary) + xtask `--mermaid`/`--theme` gen modes (`GenMode` enum) and
  mermaid/theme verify legs (`reference_export_plantuml` gained an `ext`
  param; JSON verify walk is top-level-only now) + `export_mermaid_goldens.rs`
  (3 tests: mermaid byte-parity, theme byte-parity, `mmdc` render smoke with
  the P2-D9 LOUD skip) + `bench_export` in `e2e.rs`
  (`e2e_export_plantuml_all`/`e2e_export_mermaid_all`) + `reader_m3` golden
  walk restricted to top-level `golden/*.json` (the `theme/` dir is not
  workspaces).
- **Next:** (1) `cargo xtask bench save M4` → commit the `bench-baseline/M4/`
  snapshot (the bench run takes a while — it's the criterion workspace run);
  (2) **M5** per `build/PHASE_2.md`: proptest panic suite, docs (`oxidizr
  export --help` errata fold-back), tag `v0.2.0` (LOCAL, never push), final
  RESUME rewrite. Optionally run the full `cargo xtask oracle verify` once
  before the tag (slow, ~40 s/fixture-set; CI's weekly/dispatch reference job
  runs it with `OXIDZR_REFERENCE_STRICT=1` and picks the mermaid + theme legs
  up automatically). Vendored default theme still blocked (no DNS) — D3
  interim rule.
- **Open questions:** none. Phase 1T dispatch gate **USER DECLINED push
  (2026-11-17): keep everything local** — do NOT re-ask.
- **Don't re-derive (trust):** P2-D1..D13 + decomposition (`build/PHASE_2.md`);
  `build/EXPORT-MERMAID.md` is THE Mermaid contract (the §1 format table was
  byte-confirmed by the 30 SAME witnesses — incl. rel line `1-. "…" .->2`,
  element divs `80%; margin-top:10px` / `70%; margin-top: 0px`, DN boundary
  stroke+color vs system/container stroke-as-font, group cut `#cccccc`
  fallback, `findElementStyle(tag)` None-when-undeclared chain; §11 = corpus
  exclusions); `build/EXPORT-C4-PLANTUML.md` THE C4 contract. Core gotchas:
  `Styles` fields are `element_styles`/`relationship_styles`/`themes`; merged
  relationship_style base is `style=Dashed, dashed=true`. Oracle verify
  summary line: "N json + N diagram + N theme golden set(s) match"; witnesses
  at `/tmp/nix-shell.uni100/opencode/m2d/sweep/*/ref-mermaid/` (+`ref-c4/`,
  `ref-plantuml/`); the 30-fixture list is `/tmp/nix-shell.uni100/opencode/
  m4/same-list.txt`; NEVER re-run the oracle needlessly. Golden corpus rules:
  `testdata/README.md`. Oracle = `$OXIDZR_STRUCTURIZR_BIN` (2026.10.26);
  plantuml 1.2026.9 on PATH (C4 stdlib bundled, offline); **mmdc ABSENT** (the
  mermaid render smoke LOUD-SKIPS here); NO DNS. Pinned Java
  `/tmp/nix-shell.uni100/opencode/structurizr/` `ad34036` — the reference
  decides ambiguities; fix oxidizr, never expectations/goldens. Clippy caps
  this session: `fn_params_excessive_bools` (>3 bool params → enum),
  `too_many_lines` (100 → extract-legs idiom).
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
