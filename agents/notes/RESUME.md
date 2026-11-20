# RESUME — session checkpoint (updated 2026-11-20, **P3 M4-5 DONE → M4-6 dot-backend next**)

## State

- **P2R COMPLETE** `410fee6`; **P3-M0** `346ca6e`; **P3-M1 COMPLETE**
  (`969a31b`→`496912c`); **P3-M2 COMPLETE** (`c626048`→`468ec03`,
  `P3` bench baseline); **P3-M3 COMPLETE** (`4fcb235`→`d39ebdc`);
  **M4 in flight**: M4-0 `81b0a3d`, M4-1 `60fbdc4`, M4-2 `4439931`,
  M4-3 `cba1f4b`, M4-4 `e33c675`, M4-5 `ca4221f`. **Nothing pushed;
  tags v0.1.0/v0.2.0** (v0.3.0 at P3-M5).
- **Ritual:** quartet every commit; shell commits also `cargo xtask ci
  gui`; lib commits also full tier (`OXIDZR_E2E_FULL=1 cargo test
  -p oxidizr-e2e` → 8 ok) + `cargo xtask e2e bless` → 307×unchanged +
  clean `testdata/`; commit every green sub-piece; **never push**.
- Pitfall notes written: `pitfalls/epaint-037-font-stack.md` (M4-5 —
  READ BEFORE touching fonts/export/epaint features).

## M4 remaining (§P3-M4 in PHASE_3.md — authoritative, read it)

- **M4-6 `dot-backend` feature:** `oxidizr-layout::Dot` shelling to
  `$OXIDZR_DOT_BIN` (`-Tplain`), feature OFF by default, gated test
  (Category B posture like the oracle).
- **M4 close:** PHASE_3 §P3-M4 DONE stamps + PROGRESS board/log rows +
  RESUME→M5 + full ritual.
- **M5:** `cargo xtask oracle verify` (needs `$OXIDZR_STRUCTURIZR_BIN`
  — incl. the saved-sidecar `json` load A/B leg deferred from M3-5),
  perf pass (60 fps @ 200 elements, informational), docs, tag
  **v0.3.0**.

## M4-4/M4-5 shipped surface (overview.rs + export.rs — read before touching)

- `overview::Overview::new(Scene::bounds, ERect) -> Option`: uniform
  fit + centred, degenerate inputs ⇒ None; `shapes(scene,
  view_world, selection)` = boundary strokes + item fills (selection
  accent) + viewport rect (orange); `map_to_world` inverse drives
  `Drag::Minimap` recenter (`center_world`). Quick-nav `nav_matches`
  (empty query ⇒ no hits) feeds the left-panel search; element hit =
  center+select, view hit = `show_view()`. Field `text_focus` (was
  path_focus) ORs toolbar + nav focus; guards Ctrl+Z/Y/S.
- `export::compose(scene, &mut FontsView) -> Composed{shapes,size}`:
  diagram via `paint::paint_scene` at fitted `Viewport`
  (`export_scale`: 1:1 → 4096 side cap, floor 0.25, 80 MP squeeze;
  degenerate ⇒ 400×240, selection NEVER). Key band via `key::legend`
  rows composed at ORIGIN then `Shape::translate(0, fig_h +
  KEY_TOP_PAD)` — never forget the translate. `shapes_to_svg` covers
  the closed Shape subset (Rect/Circle/Ellipse/LineSegment/Path/Text;
  Mesh/Bézier/Band/Callback skipped by contract); text = per-glyph
  outline paths from ab_glyph over the bundled Ubuntu-Light (see the
  pitfall note — workspace pin names `default_fonts`).
  `svg_to_png` = usvg parse → resvg → `encode_png`; gate = parse
  OK + PNG magic (bytes NOT goldens, P3-D0). Shell: toolbar Export ⇒
  `<stem>.svg/.png` beside `commands::save_target`, via
  `repaint_ctx.fonts_mut(...)` + `commands::write_atomic` (extracted
  from `write_workspace`, same `.{pid}.tmp`+rename — export files
  never match the watch filter, no restart_watch needed). Notes:
  success `exported …`, fail/blocked `export …` (dark-red prefix set
  now also `"export "`).

## Reload stack (M4-1..M4-3 — read before touching reload)

- `ParseResult.includes` = FS `!include` files in parse order
  (java_canonical form); `Opened.includes` carries it (empty for
  JSON/stdin).
- `reload.rs::ReloadDriver` (headless, notify, 250 ms settle-debounce,
  parent-dir NonRecursive + exact-path filter, SipHash baselines are
  the authority — events only schedule a check). `Wake = Arc<dyn Fn()
  + Send + Sync>`: eframe 0.37 has NO RepaintSignal; shell passes
  `move || ctx.request_repaint()` (cloned `egui::Context` stored as
  `repaint_ctx` on first frame). R-2 guard scans doc literals —
  reload.rs must never contain the GUI-crate names.
- Shell: poll hook + re-arm guard in `App::ui`; `adopt` drops the
  driver (`reload=None`), save Ok-arm `restart_watch()` re-baselines
  (self-save rename hash-matches ⇒ no self-reload); watch failure ⇒
  `reload_disabled` + note. `reload_from_disk` opens
  `reload_source()` = NEWEST of display/sidecar/companion .dsl; folds
  overlays into the layout side → `reconcile` → re-keys overlays via
  `Report.remapped_elements`/`remapped_relationships` (PER KIND — both
  id spaces mint "1","2",…; ambiguous/removed drop, dead views drop
  their overlay) → `adopt` → restore overlays + `rebuild_scene()`;
  failed reparse keeps last-good + note (no retry storm).
- M4-3 sidecar workflow: `open_target()` (argv + path bar) opens the
  sidecar when it exists and the `.dsl` is not strictly newer (ties →
  sidecar); `companion_dsl` keeps the `.dsl` of an opened sidecar
  watched; help overlay notes the rule.

## M3 editing core (commands.rs + shell — read before touching edits)

- `LayoutOverlay { view_key, elements (top-lefts), relationships
  (interiors), dimensions: Option<Dimensions> (Copy!) }`; readers
  `position`/`rect`/`interior` = overlay-first-then-scene;
  `as_overrides()` → `scene::build` 5th param. `Command`
  Move/SetWaypoints/SetCanvasSize/ClearView/Group, symmetric
  `apply(forward)`; factories ALL `Option<Command>` (None = no-op);
  `History` cap 200. Undo rewrites `from` geometry, map ENTRIES
  persist — identity is at RENDER level, not map equality.
- SAVE (M3-5): `apply_overlay(&Workspace, key, &overlay)` writes
  elementViews x/y + relationshipViews vertices (wholesale) +
  dimensions via `to_px` (round+clamp i32); walks the 7 MODEL-view
  vecs by `base.key` (macro `content_of_key`) — filtered/image = no
  content → key lands nowhere; shell blocks their save
  (`current_view_filtered` via `View::Filtered` + `all_views()`).
  `save_target`: `.json` in place, `.dsl` → sidecar (case-insens),
  `-` → None. `write_workspace` = `oxidizr_json::to_vec` (NO second
  serializer) + `write_atomic` (`.{pid}.tmp` → rename, shared with
  export); `SaveError{Json,Io{path, source}}`. Shell: `save_note`
  status line (dark_red iff `serialize:`/`write `/`export `
  prefixes), Ctrl+S focus-guarded, toolbar Save iff dirty; success
  clears `dirty`. Category A test pins BYTE STABILITY
  `to_vec(from_slice(bytes)) == bytes` — JSON read normalizes
  (`Some("")` descriptions, tag rebuild) + writer SORTS entry arrays
  by id ⇒ struct equality is NOT the contract.
- Shell contract: scene rebuilt WITH the overlay ⇒
  `scene.edges[i].points` IS the effective polyline; commits capture
  at press; preview frames == committed geometry (idempotent
  re-apply); `press_mode(origin, shift, alt)` handle-first; double-
  click `insert_waypoint_at`; `run_edit`/`editing_buttons`/
  `canvas_page` dialog parity; per-view overlays/histories/dirty,
  cleared at adopt only.
- `hit`: `nearest_waypoint`/`insert_waypoint` (interior-only, splice
  nearest segment), `HIT_TOLERANCE_PX 6` screen px ÷ scale;
  `paint::WAYPOINT_HANDLE_PX 4.0` filled+ring on selected Edge.

## Clippy/lint catalog (cumulative — `ci gui` alone catches shell docs)

- egui 0.37: `PointerButton::Primary`; `i.pointer.any_down()`;
  `Ui::toggle_value`; `rect_stroke(rect, rounding, stroke,
  StrokeKind)`; `Id::new` DEPRECATED (`response.has_focus()`);
  `double_clicked()`; `ui.add_enabled(bool, egui::Button::new(..))`;
  `Context::request_repaint()` IS the cross-thread wake; NO
  `strong_noninteractive_color` (use `noninteractive().fg_stroke.color`).
- `similar_names` DENY (`cursor` not `pointer` by `painter`);
  `type_complexity` ⇒ alias (`WaypointHit`) or fn-`#[allow]`+comment
  (`run_edit`); `redundant_clone` early-return; `redundant_closure`
  (also `redundant_closure_for_method_calls`: `is_some_and(T::method)`)
  ⇒ pass fn; `items_after_statements` — fn-body `use` goes FIRST;
  `assigning_clones`/`clone_on_copy` — `Dimensions` IS
  Copy; doc line starting `+` ⇒ `doc_lazy_continuation`; E0282 on
  bare `Vec::new()` in assert_eq ⇒ bind `let empty: Vec<usize>`;
  double `&mut self` one expression ⇒ split local; `cargo fmt`
  reflows — match post-fmt text; paint fixture edge has `order`
  BADGE (circle pair) — clear before counting circles.
- M4-5 round: `format_push_string` ⇒ `let _ = write!(out, …)`
  (`use std::fmt::Write as _;`); `trivially_copy_pass_by_ref` —
  `Stroke` (8 B) passes by VALUE; `f32: TryFrom<f64>` is NOT std —
  own saturating helper; `doc_markdown` flags `ab_glyph` (backtick);
  Result fns need `# Errors`; f64→f32 casts fire
  `cast_possible_truncation` (+`cast_sign_loss` for `as u32`);
  `collapsible_if` fixed via let-chains.
- R-2 guard: lib core NEVER references eframe/egui — the literals
  (also in DOC COMMENTS) fail the test; count assert is lib.rs + N
  modules (NOW 11 files: +export means keep in sync when adding a
  module).

## Viewer orientation (M1/M2 shipped)

- Scene: item rect top-left = x/y (or override), size = style else
  350×150; edge polyline = [source centre, interiors…, destination
  centre]; `hit::pick` items last-wins, edges tolerance-first;
  boundaries not pickable. `layout.rs` `stored_for`/`config_for`/
  `apply`/`encode`/`decode` (`"LR|100|50|50|1"`); boundary pad
  DEVIATION: `BOUNDARY_PADDING = 20`.
- eframe-0.37: unified `egui::Panel`; `Storage::set_string(k,
  String)`; `Window::open(&mut bool)`; `ctx.load_texture`;
  `ComboBox::from_id_salt`; click after a MOVED drag never fires;
  bless diff-0 = `cargo xtask e2e bless | grep -cE ": unchanged$"`
  == 307 + clean `git status testdata/`.
- `cargo test -p oxidizr-viewer` = 86 lib + 3 golden tests.
