# RESUME — session checkpoint (updated 2026-11-19, **P3-M0 COMPLETE → P3-M1 next**)

## State

- **P3-M0 is COMPLETE (2026-11-19).** Commits: `e3f81e6` (M0-1 plan:
  reconcile stays in `oxidizr-api`, filter::eval confirmed), `3775a38`
  (M0-2 `oxidizr-layout`: zero-dep graph + `LayoutEngine` trait, 7
  tests), `d7fcf01` (M0-3 `oxidizr-viewer`: headless lib core +
  `gui`-gated eframe shell, boundary-guard test), `c6e6bd6` (M0-4+M0-6:
  `cargo xtask ci gui` step, CI legs, AGENTS.md unsafe-scope),
  `3c742ac` (M0-5: P3-V5 CLOSED + `core::filter::eval`, 8 tests), then
  the M0-close docs commit. Every commit rode quartet + full tier 8/8 +
  bless corpus-diff=0; close bench sanity vs `2R` green. **Nothing
  pushed; tags stay v0.1.0/v0.2.0** (v0.3.0 lands at P3-M5).
- **Next: P3-M1 — read-only viewer** (`agents/plan/build/PHASE_3.md`
  §M1: open DSL/JSON, scene build, pan/zoom/select, all view kinds with
  full styling, light/dark, bundled font, icons, key/legend, tooltips,
  tag filter). M1 spans >4 modules → **charter: write the §P3-M1
  decomposition in PHASE_3.md BEFORE code**, gated on the verification
  items below.
- **P3-V items still open** (§P3-D10; empirical, BEFORE dependent code;
  monorepo clone recipe in `agents/notes/env/toolbox.md`, pinned pair
  app 2026.10.26 = lib 6.2.4 = commit `ad34036`, was checked out under
  `/tmp/opencode/structurizr-ref` — recreate if lost):
  - **P3-V2** (before M1 theming) reference font/theme loading, offline
    story → confirm the bundled-substitution policy (P3-D2).
  - **P3-V3** (before M1 icons) theme-icon corpus scan: SVG-with-text
    frequency → confirm the atlas approach.
  - **P3-V7** (before M1 tag-filter persistence choice) does the web UI
    tag filter write the view's stored `filter` object or stay
    session-only? Start at `structurizr-workspace.js:422`
    (`structurizr.filter.tags` config property) + `filter.jspf`.
  - P3-V1 (M2 `Layered` tuning), P3-V6 (M4 watch debounce), P3-V8 (M4
    export anatomy) ride later. **P3-V5 is CLOSED** — verdict + the
    cloned semantics live in `oxidizr-core::filter` rustdoc + §P3-D10.
- **Ritual unchanged:** quartet (`cargo xtask ci`) every commit;
  engine-touching commits also `OXIDZR_E2E_FULL=1 cargo test -p
  oxidizr-e2e` (expect 8 `test result: ok`) + `cargo xtask e2e bless` →
  `corpus-diff=0`; wave-close `bench cmp <label>` review; `oracle
  verify` at phase close (also weekly); commit every green sub-piece +
  rewrite this file; **never push**. Viewer legs: `cargo xtask ci gui`
  mirrors the CI clippy leg; CI also runs `cargo check -p oxidizr-viewer
  --features gui` on 3 OSes (headless — never RUNS a GUI, no xvfb).
- `redundant_clone` (nursery) is workspace-wide: write moves, or CI
  dies. House assertion style for empties: `assert_eq!(x.len(), 0)`.

## M1 orientation (read first)

- The viewer GUI stack is pinned + fetched (eframe 0.37 wgpu, notify 8,
  usvg 0.50, tiny-skia 0.12, dark-light 3 — root
  `[workspace.dependencies]` + viewer rustdoc `DEPENDENCIES` section).
- eframe 0.37 API traps already learned (see viewer `main.rs`):
  `App::ui(&mut self, ui: &mut egui::Ui, frame)` REPLACED `update`;
  `CentralPanel::default().show(ui, …)` (`show_inside` deprecated);
  `use eframe::egui` (no direct egui dep). Build/link verified on this
  box (26 s dev).
- The lib-core/GUI boundary is MACHINE-guarded
  (`oxidizr-viewer/tests` — lib sources must never reference the GUI
  tokens); M1 fills the `scene`/`hit` stubs headlessly and keeps the
  shell thin (presentation only).
- Opening files goes through `oxidizr-api::pipeline` (the viewer's
  `api` dep is ratified, §P3-D1); JSON geometry saves (M3) go through
  the `oxidizr-json` writer — WRITE-CATALOG rules unchanged.
- Tag-filter parity: `oxidizr-core::filter::eval` implements the
  reference semantics already — reuse it for both the DSL FilteredView
  and (post-V7) the session tag filter.
