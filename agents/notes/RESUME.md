# RESUME — session checkpoint (updated 2026-11-20, **P3 M3 COMPLETE → M4 live reload + export next**)

## State

- **P2R COMPLETE** `410fee6`; **P3-M0** `346ca6e`; **P3-M1 COMPLETE**
  (`969a31b`→`496912c`); **P3-M2 COMPLETE** (`c626048`→`468ec03`,
  `P3` bench baseline); **P3-M3 COMPLETE** (`4fcb235`→this close):
  M3-2 `6b401c0`, M3-3 `c0ad59e`, M3-4 `6c5d541`, M3-5 `1d58964`.
  **Nothing pushed; tags v0.1.0/v0.2.0** (v0.3.0 at P3-M5).
- **Ritual:** quartet every commit; shell commits also `cargo xtask ci
  gui`; lib commits also full tier (`OXIDZR_E2E_FULL=1 cargo test
  -p oxidizr-e2e` → 8 ok) + `cargo xtask e2e bless` → 307×unchanged +
  clean `testdata/`; commit every green sub-piece; **never push**.
- `bench cmp P3` at M3 close: ALL groups 1.00 (engine untouched).

## M4 NEXT (§P3-M4 in PHASE_3.md — authoritative, read it)

- **Live reload + remap (P3-D6, V6 gate):** `reload.rs` is still the
  M0 stub (`ReloadDriver` placeholder). Wire a notify watcher on the
  opened file + `!include` closure, 250 ms debounce, reparse through
  `oxidizr_api::open`, remap old→new ids via the SHARED
  `oxidizr-api::reconcile` engine (stays in api, viewer consumes),
  keep surviving overlay geometry. V6 = per-OS notify coalescing +
  our own temp+rename writer must not storm (M3-5 writes
  `{name}.{pid}.tmp` then renames — the watcher must ignore temps and
  survive rename-as-modify). Display string after adopt already
  canonicalized (`Opened.display`, `-` = stdin).
- **DSL→sidecar workflow:** M3-5 save already produces `<stem>.json`;
  the reload watch should ALSO watch the sidecar once it exists
  (decide: prefer sidecar over DSL for reload once saved).
- **Minimap (navigator.jspf) + quick-nav (quick-navigation.jspf).**
- **Export SVG+PNG with key (P3-V8 reference look, Q2):** headless
  ride on `paint::paint_scene` shapes → SVG (usvg-free? decide) / PNG
  (software raster — epaint has no rasterizer: decide tessellator +
  miniquad-free path, this is the meaty design point).
- **`dot-backend` feature:** optional `graphviz dot` engine behind
  `$OXIDZR_DOT_BIN` (Category B posture like the oracle).
- M4 gates the same quartet/`ci gui`/full tier/bless ritual; watch
  tests = temp dir + fs events, NO sleeps longer than needed (CI).

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
  `-` → None. `write_workspace`: `oxidizr_json::to_vec` (NO second
  serializer) → `.{pid}.tmp` → rename; `SaveError{Json,Io{path,
  source}}`. Shell: `save_note` status line (dark_red iff
  `serialize:`/`write ` prefixes), Ctrl+S focus-guarded, toolbar Save
  iff dirty; success clears `dirty`. Category A test pins BYTE
  STABILITY `to_vec(from_slice(bytes)) == bytes` — JSON read
  normalizes (`Some("")` descriptions, tag rebuild) + writer SORTS
  entry arrays by id ⇒ struct equality is NOT the contract.
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
  `double_clicked()`; `ui.add_enabled(bool, egui::Button::new(..))`.
- `similar_names` DENY (`cursor` not `pointer` by `painter`);
  `type_complexity` ⇒ alias (`WaypointHit`) or fn-`#[allow]`+comment
  (`run_edit`); `redundant_clone` early-return; `redundant_closure`
  ⇒ pass fn; `assigning_clones`/`clone_on_copy` — `Dimensions` IS
  Copy; doc line starting `+` ⇒ `doc_lazy_continuation`; E0282 on
  bare `Vec::new()` in assert_eq ⇒ bind `let empty: Vec<usize>`;
  double `&mut self` one expression ⇒ split local; `cargo fmt`
  reflows — match post-fmt text; paint fixture edge has `order`
  BADGE (circle pair) — clear before counting circles.
- R-2 guard: lib core NEVER references eframe/egui (9 files checked;
  `oxidizr_json`/`oxidizr_api` imports fine).

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
- `cargo test -p oxidizr-viewer` = 71 lib + 3 golden tests.

## M5 (after M4)

- `cargo xtask oracle verify` (needs `$OXIDZR_STRUCTURIZR_BIN`) —
  the saved-sidecar `json` load A/B leg was deferred here from M3-5;
  perf pass (60 fps @ 200 elements, informational), docs, tag
  **v0.3.0**.
