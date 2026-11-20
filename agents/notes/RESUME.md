# RESUME — session checkpoint (updated 2026-11-20, **P3 M3-5 done → M3 close next**)

## State

- **P2R COMPLETE** `410fee6`; **P3-M0** (`e3f81e6`→`346ca6e`);
  **P3-M1 COMPLETE** (`969a31b`→`496912c`); **P3-M2 COMPLETE**
  (`c626048`→`e647dcd`, close `468ec03` + `P3` bench baseline);
  **P3-M3**: M3-1 `4fcb235`, M3-2 `6b401c0`, M3-3 `c0ad59e`,
  M3-4 = this commit. **Nothing pushed; tags v0.1.0/v0.2.0**
  (v0.3.0 at P3-M5).
- **Ritual:** quartet every commit; shell commits also `cargo xtask ci
  gui`; lib commits also full tier (`OXIDZR_E2E_FULL=1 cargo test
  -p oxidizr-e2e` → 8 ok) + `cargo xtask e2e bless` → 307×unchanged +
  clean `testdata/`; **never push**.
- **M3 plan (§P3-M3 table in PHASE_3.md — authoritative, read it):**
  - **M3-4 DONE (this commit)** waypoint editing + parity UI:
    - `hit::nearest_waypoint(point, polyline, tol)` → polyline index of
      the nearest INTERIOR vertex (endpoints skipped, tol world px)
      and `hit::insert_waypoint(point, polyline)` → splice index
      (split nearest segment via `distance_to_segment`).
    - `commands::drag_waypoint(id, from, index, delta, snap)` — press-
      captured INTERIOR contract, idempotent like `drag_move`.
    - `paint::WAYPOINT_HANDLE_PX = 4.0`; selected `Selection::Edge`
      paints filled+ring circles per interior vertex (screen px).
    - Shell: `Drag::Waypoint { id, from, index, press, delta }`;
      `press_mode(origin, shift, alt)` — handle of the SELECTED edge
      grabs FIRST (Alt+press deletes vertex via `set_waypoints`,
      plain press drags; preview = `preview_waypoint`, release commits
      `drag_waypoint`, None → restore press route); double-click on an
      edge → `insert_waypoint_at` (pick Edge, splice, `set_waypoints`);
      dialog footer `editing_buttons` row (Align L/Cx/R/T/Cy/B ≥2,
      Space ↔/↕ ≥3, Centre, Bring back via `canvas_page()` = visible
      canvas WORLD rect, Remove vertices) — all through `run_edit`
      (overlay clone → factory → `edit_current_view`); `WaypointHit`
      type alias for the hit tuple; help text updated.
  - **M3-5 DONE (this commit)** SAVE:
    - `commands::apply_overlay(&Workspace, view_key, &LayoutOverlay) ->
      Workspace` — clone-and-write into `elementViews x/y`,
      `relationshipViews vertices` (wholesale), view `dimensions`;
      `to_px` rounds+clamps to the stored i32 grid; walks `ViewSet`
      via local `content_of_key` (the 7 MODEL-view vecs by
      `base.key` — filtered/image have NO content → key lands
      nowhere); model/styles/automaticLayout untouched (V4).
    - `commands::save_target(display)`: `.json` in place, `.dsl` →
      `<stem>.json` sidecar (case-insensitive), `"-"` stdin → None.
    - `commands::write_workspace(path, ws)`: `oxidizr_json::to_vec`
      (NO second serializer) → `{name}.{pid}.tmp` → rename (P3-D6);
      `SaveError { Json(#[from] JsonError), Io { path, source } }`.
    - Shell: `save_note: Option<String>` (status bar: dark_red only on
      `serialize:`/`write ` prefixes, else dark_green; cleared per
      attempt + at adopt); `save_workspace()` blocks on
      `current_view_filtered()` (`View::Filtered` via `all_views()`)
      and stdin, folds ALL dirty overlays, success → `dirty.clear()`;
      Ctrl+S in the undo/redo block (path-focus guarded); toolbar
      "Save" enabled iff dirty non-empty; help text lists Ctrl+S.
    - Category A test: open temp DSL → ids from FIRST view content →
      overlay (222.4,333.6)→(222,334) rounds → ghost key == workspace
      → sidecar write → `from_slice` → **byte-stability** re-write
      `to_vec(parsed) == bytes` (NOT struct equality: JSON read
      normalizes absent description → `Some("")` + rebuilds tags, the
      writer sorts entry arrays by id — idempotent saves is the
      contract). `save_target` unit test.
    - Also: hit.rs lasso-empty test needed `let empty: Vec<usize>`
      (E0282 with the new generic inference context).
  - **M3 CLOSE NEXT**: PROGRESS board + log M3 row; PHASE_3 M3 DONE
    stamps; RESUME → M4 entry; bench sanity = `cmp P3` spot-check
    only (engine untouched).
  - V4 verdict (§P3-D10): stored `automaticLayout` block PERMANENT,
    honored at render, we stay EDITABLE; save writes GEOMETRY ONLY,
    never the block.
  - Scope cuts: SetAnchor + width/height deltas → P4; undo per-view.
- **M4** (P3-D9): live reload + remap UX (notify, 250 ms debounce,
  api::reconcile remap, temp+rename no-storm), DSL→sidecar workflow,
  minimap + quick-nav, export SVG+PNG, `dot-backend` feature
  (`$OXIDZR_DOT_BIN`). V6 (notify coalescing per OS) gates the watch.
- **M5**: `cargo xtask oracle verify` (needs `$OXIDZR_STRUCTURIZR_BIN`),
  perf pass, docs, tag **v0.3.0**.

## M3 editing core (commands.rs — read before M3-5)

- `LayoutOverlay { view_key, elements, relationships, dimensions }`
  (`for_view`, `is_empty`, `as_overrides`, `position`/`rect`/
  `interior` read overlay-first-then-scene); `Command`
  (MoveElements{moves}/SetWaypoints{changes}/SetCanvasSize{from,to}/
  ClearView{snapshot}/Group{steps}) with symmetric `apply(forward)`.
  Factories `move_by`(snap, GRID_PX 10), `align`(AlignEdge l/cx/r/t/
  cy/b, bbox target, ≥2), `distribute`(Axis, ≥3, extremes fixed),
  `centre_view`, `bring_back`(page Rect, minimal delta),
  `remove_all_vertices`, `set_waypoints`(from = overlay.interior),
  `drag_waypoint`, `set_canvas_size`, `clear_view`, `drag_target`/
  `drag_move`, `apply_layout(scene, overlay, overrides)` →
  Moves/Changes/Group — ALL `Option<Command>` (None = no-op, never
  pushed). `History` run/undo/redo/reset/can_*, HISTORY_CAP 200.
- Undo semantics (pinned): undo REWRITES `from` geometry, map ENTRIES
  persist — round-trip identity is at RENDER level (`position()`/
  `interior()` readers), not map equality.
- Fixture math (commands tests): 3 boxes 100×50 at (0,0)/(220,60)/
  (450,0), edge10 interiors [(120,40),(200,80)], bounds (-20,-20,
  590,150); `drag_waypoint(10, +[15,5], snap)` → vertex (220,90);
  distribute gap 125 → box2 225; drag_move((0,0)+(15,5),snap)→(20,10).
- Shell contract: scene is rebuilt WITH the overlay, so
  `scene.edges[i].points` IS the effective polyline — hit tests and
  handle painting read the scene, commits capture at press.

## M2 auto-layout (shipped)

- `layout.rs` (headless, 7 tests): `stored_for` (Filtered → BASE view;
  `content.automatic_layout` — a **ViewContent** field), `config_for`
  (null direction ⇒ LR), `apply` (placements→top-lefts, route
  interiors, empty straightens), `encode`/`decode`
  (`"LR|100|50|50|1"`, garbage→None).
- `scene::build(ws, key, scheme, mask, Option<&Overrides>)`; overrides
  REPLACE (elements = TOP-LEFT; relationships = interiors wholesale).
  DEVIATION (M2-5 row): boundary pad stays `BOUNDARY_PADDING = 20`.

## Clippy/lint catalog (cumulative — the gui leg alone catches shell docs)

- egui 0.37: `PointerButton::Primary` (NOT `Left`); no
  `InputState::any_pointer_down` ⇒ `i.pointer.any_down()`;
  `Ui::toggle_value(&mut bool, atoms)`; `Painter::rect_stroke(rect,
  rounding, stroke, StrokeKind)`; `Id::new` DEPRECATED ⇒ focus via
  `response.has_focus()` into a flag; `Response::double_clicked()`.
- `similar_names` DENY — `pointer` vs existing `painter` (use
  `cursor`). `type_complexity` ⇒ alias (`WaypointHit`) or fn-level
  `#[allow]` + comment (run_edit's factory param). `redundant_clone`
  in an early-return branch (move `from`, don't clone).
  `redundant_closure` ⇒ pass the fn (`run_edit(remove_all_vertices)`).
- Doc line starting `+` ⇒ `doc_lazy_continuation`; `[] as Vec` cast
  ILLEGAL ⇒ `[] as [(f64,f64); 0]`; `float_cmp` allow where exactness
  by design; `manual_midpoint` ⇒ `f64::midpoint`;
  `cast_precision_loss` ⇒ fn-level allow + comment; double `&mut self`
  in one expression ⇒ split with a local; `cargo fmt` reflows
  signatures — match post-fmt text.
- Paint fixtures bite: the paint-test edge has an `order` BADGE
  (circle pair) — clear `order = None` before counting circles.

## M1 viewer orientation (shipped)

- eframe/egui/epaint 0.37: `App::ui(&mut self, ui, frame)`; unified
  `egui::Panel`; `Storage::set_string(k, String)`; `Window::open(&mut
  bool)`; `ctx.load_texture`/`Painter::image`; `ComboBox::from_id_salt`,
  `ui.selectable_value`, `DragValue::range(0..=1000)` (i32).
- Click after a DECIDED drag (that MOVED) never fires; plain
  press-release still clicks (press-decided mode is click-safe).
- Scene: item rect top-left = element x/y (or override), size = style
  else 350×150; edge polyline = [source centre, vertices…, destination
  centre]; `hit::pick` items last-wins, edges by tolerance, boundaries
  NOT pickable; `Hit::Item/Edge(usize)` = scene-layer indices.
- bless diff-0: `cargo xtask e2e bless 2>&1 | grep -cE ": unchanged$"`
  = 307 + clean `git status` under `testdata/`.
- `cargo test -p oxidizr-viewer` = 69 lib + 3 golden tests.
