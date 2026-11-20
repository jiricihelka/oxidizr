# RESUME — session checkpoint (updated 2026-11-20, **P3 M4 COMPLETE → M5 hardening next**)

## State

- **P2R COMPLETE** `410fee6`; **P3-M0** `346ca6e`; **P3-M1 COMPLETE**
  (`969a31b`→`496912c`); **P3-M2 COMPLETE** (`c626048`→`468ec03`,
  `P3` bench baseline); **P3-M3 COMPLETE** (`4fcb235`→`d39ebdc`);
  **P3-M4 COMPLETE** (`81b0a3d`→`dc5d840`): M4-0 `81b0a3d`, M4-1
  `60fbdc4`, M4-2 `4439931`, M4-3 `cba1f4b`, M4-4 `e33c675`, M4-5
  `ca4221f`, M4-6 `dc5d840`. **Nothing pushed; tags v0.1.0/v0.2.0**
  (v0.3.0 is the M5 close).
- **Ritual:** quartet every commit; shell commits also `cargo xtask ci
  gui`; lib commits also full tier (`OXIDZR_E2E_FULL=1 cargo test
  -p oxidizr-e2e` → 8 ok) + `cargo xtask e2e bless` → 307×unchanged +
  clean `testdata/`; commit every green sub-piece; **never push**.

## M5 (§P3-M5 in PHASE_3.md — authoritative, read it first)

- `cargo xtask oracle verify` green — incl. the deferred leg: a
  viewer-saved sidecar JSON must load through the reference (M3-5
  posture — save wrote bytes no reference ever parsed).
- Perf pass: 60 fps @ 200 elements — informational (P3-D0), record
  what you see.
- Docs refresh (viewer usage + README/diagnostics if it names the
  viewer), then **tag v0.3.0** (local, no push).

## M4-6 dot-backend surface (dot.rs + layout.rs — read before touching)

- `oxidizr-layout` feature `dot-backend` (OFF by default, adds only
  `std::process`): `Dot::from_env()` = `$OXIDZR_DOT_BIN` (empty ⇒
  None, oracle posture); `dot_source()` = input order, positional
  names `n0..` (ids NEVER enter the source), graph attrs
  rankdir/nodesep/ranksep/esep (÷ 72 px/in), splines from
  `generate_vertices`, tailport/headport for anchors.
- **`-Tplain` reality (graphviz 15.1.1, verified 2026-11-20 — NOT the
  man page):** node rows carry FULL extents (top-left = `(x-w/2,
  graph_h-y-h/2)`), edge points are TWO plain numbers (docs' joined
  `x,y` also parsed), the FIRST `graph` block starts at the origin
  (gap only BETWEEN components, 50 px), y is bottom-up, inches.
- `parse_plain` maps back in input order; parallel edges FIFO per
  (src,dst) pair. Category B test `real_dot_binary…` gated on the env
  var; determinism = fixed graphviz build.
- Viewer: feature passthrough `dot-backend`; `layout::Engine`
  (Layered always; Dot only feature + env) — `available()` powers the
  dialog combo (appears iff > 1 engine), `apply_with()` is the one
  translation pipeline (`apply()` = Layered, headless tests
  unchanged); Dot w/o env ⇒ `LayoutError::DotBackend` value. Engine
  choice SESSION-ONLY — `encode`/`decode` stay `"LR|100|50|50|1"`.

## M4-4/M4-5 shipped surface (overview.rs + export.rs)

- `overview::Overview::new(Scene::bounds, ERect) -> Option`: uniform
  fit + centred; `shapes(scene, view_world, selection)`; `map_to_world`
  inverse drives `Drag::Minimap` recenter. Quick-nav `nav_matches`
  feeds the left-panel search; field `text_focus` ORs toolbar + nav
  focus; guards Ctrl+Z/Y/S.
- `export::compose(scene, &mut FontsView) -> Composed{shapes,size}`:
  fitted `Viewport` (1:1 → 4096 cap, floor 0.25, 80 MP; degenerate ⇒
  400×240; selection NEVER painted); key band composed at ORIGIN then
  `Shape::translate(0, fig_h + KEY_TOP_PAD)` — never forget the
  translate. `shapes_to_svg` covers the closed Shape subset; text =
  per-glyph outlines via `ab_glyph` over the bundled Ubuntu-Light
  (workspace pin names `default_fonts` — READ
  `pitfalls/epaint-037-font-stack.md` before touching fonts/epaint).
  `svg_to_png` = usvg → resvg → `encode_png`; gate = parse OK + PNG
  magic (bytes NOT goldens, P3-D0). Shell: toolbar Export ⇒
  `<stem>.svg/.png` beside `commands::save_target` via
  `commands::write_atomic` (shared with save; `.{pid}.tmp`+rename
  never matches the watch filter). Notes: success `exported …`,
  fail/blocked `export …` (dark-red prefix set: `serialize:`,
  `write `, `export `).

## Reload stack (M4-1..M4-3 — read before touching reload)

- `ParseResult.includes` = FS `!include` files in parse order;
  `Opened.includes` carries it (empty for JSON/stdin).
- `reload.rs::ReloadDriver` (headless, notify, 250 ms settle-debounce,
  parent-dir NonRecursive + exact-path filter, SipHash baselines are
  the authority). `Wake = Arc<dyn Fn() + Send + Sync>` (eframe 0.37
  has NO RepaintSignal; shell passes `move || ctx.request_repaint()`,
  `repaint_ctx` stored first frame). R-2 guard scans doc literals.
- Shell: poll hook + re-arm guard in `App::ui`; `adopt` drops the
  driver; save Ok-arm `restart_watch()` re-baselines (self-save
  hash-matches ⇒ no self-reload); watch failure ⇒ `reload_disabled`.
  `reload_from_disk` opens `reload_source()` = NEWEST of
  display/sidecar/companion `.dsl` (ties ⇒ sidecar); folds overlays →
  `reconcile` → re-keys overlays via `Report.remapped_elements`/
  `remapped_relationships` (PER KIND) → `adopt` → restore overlays +
  `rebuild_scene()`; failed reparse keeps last-good + note.
  `companion_dsl` keeps an opened sidecar's `.dsl` watched.

## M3 editing core (commands.rs + shell — read before touching edits)

- `LayoutOverlay { view_key, elements (top-lefts), relationships
  (interiors), dimensions: Option<Dimensions> (Copy!) }`; readers
  `position`/`rect`/`interior` overlay-first; `as_overrides()` →
  `scene::build` 5th param. `Command` set symmetric `apply(forward)`;
  factories ALL `Option<Command>`; `History` cap 200; per-view
  overlay/history/dirty, cleared at adopt only.
- SAVE (M3-5): `apply_overlay` writes elementViews x/y +
  relationshipViews vertices + dimensions (`to_px` round+clamp i32),
  walks the 7 MODEL-view vecs by `base.key`; filtered/image ⇒ save
  BLOCKED (`current_view_filtered`). `save_target`: `.json` in place,
  `.dsl` → sidecar, `-` → None. `write_workspace` =
  `oxidizr_json::to_vec` + `write_atomic`. BYTE STABILITY test:
  `to_vec(from_slice(bytes)) == bytes` (writer sorts by id — struct
  equality is NOT the contract). Ctrl+S focus-guarded; Save iff dirty.

## Clippy/lint catalog (cumulative — `ci gui` alone catches shell docs)

- egui 0.37: `PointerButton::Primary`; `i.pointer.any_down()`;
  `Ui::toggle_value`; `rect_stroke(rect, rounding, stroke,
  StrokeKind)`; `Id::new` DEPRECATED (`response.has_focus()`);
  `double_clicked()`; `ui.add_enabled(bool, egui::Button::new(..))`;
  `Context::request_repaint()` IS the cross-thread wake; NO
  `strong_noninteractive_color`; `Shape::translate(&mut self, Vec2)`;
  `CornerRadius` = `nw/ne/sw/se: u8`; `Shape::line` = OPEN Path.
- `similar_names` DENY; `type_complexity` ⇒ alias or fn-`#[allow]` +
  comment; `redundant_clone` early-return; `redundant_closure`
  (is_some_and(T::method)); `items_after_statements` — fn-body `use`
  goes FIRST; `assigning_clones`/`clone_on_copy` — `Dimensions` IS
  Copy; `doc_lazy_continuation` for doc lines starting `+`; E0282 on
  bare `Vec::new()` in assert_eq ⇒ bind typed local; `cargo fmt`
  reflows — match post-fmt text.
- `format_push_string` ⇒ `let _ = write!`/`writeln!` (a format ending
  in `\n` MUST be `writeln!`); `trivially_copy_pass_by_ref` — `Stroke`
  (8 B) by VALUE; `f32: TryFrom<f64>` is NOT std — own saturating
  helper; `doc_markdown` backticks (`ab_glyph`); Result fns need
  `# Errors`; `collapsible_if` via let-chains; `too_many_lines` allow
  needs a rationale comment; unused `mut` when a cfg block vanishes —
  restructure instead of `#[allow]`.
- R-2 guard: lib core NEVER references eframe/egui (literals in DOC
  COMMENTS fail); count assert = lib.rs + N modules — NOW 11 files.

## Viewer orientation (M1/M2 shipped)

- Scene: item rect top-left x/y (or override), size = style else
  350×150; edge polyline = [source centre, interiors…, destination
  centre]; `hit::pick` items last-wins, edges tolerance-first;
  boundaries not pickable. `layout.rs` `stored_for`/`config_for`/
  `apply`/`encode`/`decode`; boundary pad `BOUNDARY_PADDING = 20`
  (deviation). Core↔engine direction names differ — `layout::
  direction()` maps.
- eframe-0.37: unified `egui::Panel`; `Storage::set_string(k, String)`;
  `Window::open(&mut bool)`; `ctx.load_texture`;
  `ComboBox::from_id_salt`; click after a MOVED drag never fires;
  bless diff-0 = `cargo xtask e2e bless | grep -cE ": unchanged$"` ==
  307 + clean `git status testdata/`.
- `cargo test -p oxidizr-viewer` = 89 lib + 3 golden tests;
  `cargo test -p oxidizr-layout --features dot-backend` = 31.
