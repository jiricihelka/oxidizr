# RESUME — session checkpoint (updated 2026-11-20, **P3-M1 in progress → M1-4 next**)

## State

- **P2R COMPLETE** `410fee6`; bench baseline label `2R` (`6baa7b4`).
  **P3-M0 COMPLETE** (commits `e3f81e6`→`346ca6e`: layout crate, viewer
  crate + boundary guard, `ci gui` leg, `core::filter::eval` = V5).
  **Nothing pushed; tags stay v0.1.0/v0.2.0** (v0.3.0 lands at P3-M5).
- **P3-M1 (read-only viewer)** — decomposition §P3-M1 in
  `agents/plan/build/PHASE_3.md` (pre-work `d95705b`: P3-V2/V3/V7 CLOSED
  + the M1-1…M1-5 rows written). Order: M1-1 → M1-2 → M1-3 → M1-4 →
  M1-5 → close.
  - **M1-1 DONE** `969a31b`: `oxidizr-api::open()` — partial-DSL open,
    validate tail informational, 7 tests (`tests/open.rs`).
  - **M1-2 DONE 2026-11-20**: `oxidizr-viewer/src/scene.rs` headless
    scene core, 14 unit tests. `build(&Workspace, view_key,
    ColorScheme, Option<&Included>) -> Result<Scene, SceneError>`;
    `Scene { view_key, title, background, boundaries, items, edges,
    bounds }`. Deviation recorded on the §P3-M1 row: text = plain
    `String` runs, epaint layout jobs deferred to M1-4 (no epaint dep,
    guard unchanged). `thiserror` is now a viewer-lib dep.
  - **M1-3 DONE 2026-11-20**: `src/hit.rs` — `Viewport` (screen =
    world*scale + offset; `MIN/MAX_SCALE` 0.05/20 NaN-safe; pivot-
    preserving `zoom_at`; `fit` + `FIT_PADDING_PX` 40), `pick` (items
    last-drawn win, then FIRST edge within `HIT_TOLERANCE_PX` 6 px ÷
    scale; boundaries not pickable until M3), `distance_to_polyline`/
    `_segment` degenerate-safe; 10 unit tests incl. clamp bounds +
    zero-length segments.
  - **M1-4 NEXT**: gui shell (open argv/menu, view list, pan/zoom/select
    via `hit`, full styling paint incl. opacity shading + the epaint
    text layout deferred from M1-2, light/dark + `dark-light`, tooltips,
    key panel, session tag-filter chips via `filter::eval`, custom/image
    placeholder) — big-bang commit, split canvas-vs-overlays seam if
    big; gate also `cargo xtask ci gui`. M1-5 icons (usvg 0.50 +
    tiny-skia 0.12 atlas, placeholder on missing).
- **P3-V items**: V2/V3/V5/V7 CLOSED (verdicts in §P3-D10 + code
  rustdoc); **open: V1 (M2 Layered), V4 + V6 (M3), V8 (M4)**. Monorepo
  clone (`ad34036`) under `/tmp/opencode/structurizr-ref` — recreate per
  `agents/notes/env/toolbox.md` if lost.
- **Ritual unchanged:** quartet (`cargo xtask ci`) every commit;
  engine/API-touching commits also `OXIDZR_E2E_FULL=1 cargo test -p
  oxidizr-e2e` (8 `test result: ok`) + `cargo xtask e2e bless` →
  corpus-diff=0; viewer-gui commits also `cargo xtask ci gui`; commit
  every green sub-piece + rewrite this file; **never push**.

## M1 orientation (read first)

- GUI stack pinned (eframe 0.37 wgpu, notify 8, usvg 0.50, tiny-skia
  0.12, dark-light 3) — viewer lib.rs `DEPENDENCIES` rustdoc. eframe
  0.37 traps (`main.rs`): `App::ui(&mut self, ui, frame)` REPLACED
  `update`; `CentralPanel::default().show(ui, …)`; `use eframe::egui`.
- Boundary guard (`lib.rs` tests): lib sources never reference
  eframe/egui tokens, asserts exactly 5 lib `.rs` files (lib + scene +
  hit + commands + reload). Headless core = presentation-ready data;
  the shell only paints/picks events.
- `scene::build` invariants (module docs are authoritative): stored
  top-left `x/y`; size = style width/height else 350×150; edges = src
  centre → stored vertices → dst centre; boundaries = member unions +
  `BOUNDARY_PADDING`, ordered by first-member content index then id;
  stale element/relationship ids skipped; container-view
  `removeIllegalElements` mirrored; titles/metadata/symbols per
  `structurizr-ui.js` (`getTitleForView`, `getMetadataForElement` with
  technology suffix, `getTerminologyFor`, `metadataSymbols` default
  SquareBrackets).
- Core view structs have NO `Default` — build every field (idiom in
  `core::filter` tests + `scene.rs` tests); `View<'a>` is `Copy` but
  `content()` borrows the enum binding — keep a resolved `base_view:
  View` binding alive while using its content (E0597 trap).
- Clippy house traps (nightly pin): doc-list continuation lines need
  exactly `//!   ` (3 spaces; 4+ = `doc_overindented`, and never start a
  continuation with `+`); `assert!(x.is_empty())` is BANNED
  (`assert_is_empty` pedantic) → `assert_eq!(x, [] as [T; 0])` (shows
  value on failure) or `assert_eq!(x.len(), 0)`; `redundant_clone`
  (nursery) — write moves; `too_many_lines`/`too_many_arguments` get
  justified `#[allow]` only where the shape IS the contract.
- Opening files goes through `oxidizr-api::open` (§P3-D1); JSON geometry
  saves (M3) through the `oxidizr-json` writer — WRITE-CATALOG rules
  unchanged. Tag filter = `oxidizr_core::filter::eval` for both the DSL
  FilteredView (resolved inside `build`) and the session chips (V7:
  session-only, never writes the view's stored filter).
