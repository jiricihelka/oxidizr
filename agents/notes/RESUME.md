# RESUME — session checkpoint (updated 2026-11-20, **P3 M3-3 done → M3-4 next**)

## State

- **P2R COMPLETE** `410fee6`; **P3-M0** (`e3f81e6`→`346ca6e`);
  **P3-M1 COMPLETE** (`969a31b`→`496912c`); **P3-M2 COMPLETE**
  (`c626048`→`e647dcd`, close `468ec03` + `P3` bench baseline);
  **P3-M3**: M3-1 `4fcb235`, M3-2 `6b401c0`, M3-3 = this commit.
  **Nothing pushed; tags v0.1.0/v0.2.0** (v0.3.0 at P3-M5).
- **Ritual:** quartet every commit; shell commits also `cargo xtask ci
  gui`; lib commits also full tier (`OXIDZR_E2E_FULL=1 cargo test
  -p oxidizr-e2e` → 8 ok) + `cargo xtask e2e bless` → 307×unchanged +
  clean `testdata/`; **never push**.
- **M3 plan (§P3-M3 table in PHASE_3.md — authoritative, read it):**
  - **M3-3 DONE (this commit)** shell editing interactions:
    - `Viewer` fields: `overlays: HashMap<String, LayoutOverlay>` +
      `histories: HashMap<String, History>` (PER VIEW — survive view
      switches, cleared at adopt) + `snap: bool` + `drag: Drag` +
      `dirty: BTreeSet<String>` + `path_focus: bool`;
      `layout_overrides` is GONE; `rebuild_scene` feeds the current
      overlay via `as_overrides()`.
    - Every geometry mutation lands via `edit_current_view(command)`
      (history.run + dirty.insert + rebuild); `step_history(forward)` =
      per-view Ctrl+Z/Y; `overlay_for()` = entry-or-create.
    - `Drag` state machine in `canvas()`: MODE decided at press —
      Middle or Space+Primary → `Pan`; Primary on a box → `press_mode`
      → `Move { origins (effective top-lefts, content order, captured
      at press), press (WORLD), delta }`; Primary on background/edge →
      `Lasso { anchor: Pos2 (screen), base }`. `release` = !dragged &&
      drag!=Idle && !pointer.any_down. Preview writes
      `commands::drag_target(origin, delta, snap)` per frame → preview
      frames EQUAL the commit (idempotent re-apply); release commits
      `commands::drag_move`, None → restore origins + rebuild. Lasso =
      `hit::items_in_rect` (world rect, INTERSECTS, touching does NOT
      select), Shift extends `base`; rubber band painted with
      `painter.rect_stroke(..., StrokeKind::Inside)`.
    - `canvas_click(point, command, shift, ctx)`: Ctrl/⌘ URL → open;
      Shift-click TOGGLES membership; plain click replaces; empty +
      !shift clears. `paint::Selection::Items(Vec<Id>)` REPLACES
      `Item(Id)` (paint accent = `ids.contains`; status shows count).
    - Keys: Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y → `step_history`; skipped
      while `path_focus` (toolbar sets it from `edit.has_focus()` —
      egui 0.37 DEPRECATED `Id::new`, so no PATH_EDIT widget-id const).
    - Toolbar `Snap` toggle (`ui.toggle_value`); view_list shows `●`
      for dirty views; `run_layout` lands the engine output as ONE
      undoable `apply_layout` command (dialog Apply AND adopt-time
      autolayout_once); dialog Reset = `clear_view` command (split the
      `self.x(self.y())` double-borrow with a local).
    - New headless helpers this commit: `hit::items_in_rect`,
      `Command::Group { steps }` (apply in order, undo reversed),
      `drag_target`/`drag_move`/`apply_layout` factories; lib = 65
      tests green.
  - **M3-4 NEXT** waypoint handles (drag/insert/Alt-delete) + dialog
    footer buttons (magnet bring-back, remove-vertices) + align/
    distribute row — factories ALL exist in commands.rs already
    (`set_waypoints`, `bring_back`, `remove_all_vertices`, `align`,
    `distribute`, `centre_view`); this is UI + handle hit-testing.
  - **M3-5** SAVE: `apply_overlay` (clone workspace, write x/y +
    vertices + dimensions) → `oxidizr_json::to_vec` → temp+rename;
    Ctrl+S; `.json` in place, DSL → `<stem>.json` sidecar; filtered
    views save-blocked; Category A round-trip test. Remember: undo
    leaves map ENTRIES at stored values (is_empty is map-based) —
    save guard must compare geometry, not `is_empty` alone.
  - V4 verdict (in §P3-D10): block permanent, honored, we stay EDITABLE
    (overlay wins session, save writes GEOMETRY ONLY, never the block);
    reference re-lays-out every render + locks (`:355/:400/:989`);
    `removeAllVertices` before EVERY apply (overwrite always).
  - Scope cuts: SetAnchor + width/height deltas → P4; undo per-view.
- **M4** (P3-D9): live reload + remap UX (notify, 250 ms debounce,
  api::reconcile remap, temp+rename no-storm), DSL→sidecar workflow,
  minimap + quick-nav, export SVG+PNG, `dot-backend` feature
  (`$OXIDZR_DOT_BIN`). V6 (notify coalescing per OS) gates the watch.
- **M5**: `cargo xtask oracle verify` (needs `$OXIDZR_STRUCTURIZR_BIN`),
  perf pass, docs, tag **v0.3.0**.

## M3 editing core (commands.rs — read before M3-4)

- `LayoutOverlay { view_key, elements, relationships, dimensions }`
  (`for_view`, `is_empty`, `as_overrides`, `position`/`rect`/
  `interior` read overlay-first-then-scene); `Command`
  (MoveElements{moves}/SetWaypoints{changes}/SetCanvasSize{from,to}/
  ClearView{snapshot}/Group{steps}) with symmetric `apply(forward)` —
  `from`/`to` travel in the command, no closures. Factories
  `move_by`(snap, GRID_PX 10), `align`(AlignEdge l/c/r/t/m/b, bbox
  target, ≥2), `distribute`(Axis, ≥3, extremes fixed), `centre_view`,
  `bring_back`(page Rect, minimal delta), `remove_all_vertices`,
  `set_waypoints`, `set_canvas_size`, `clear_view`, `drag_move` (froms
  captured at press, idempotent), `apply_layout(scene, overlay,
  overrides)` → Moves/Changes/Group — ALL return `Option<Command>`
  (None = no-op, never pushed). `History` run/undo/redo/reset/can_*,
  HISTORY_CAP 200 (drop-oldest, run clears redo).
- Undo semantics (pinned): undo REWRITES `from` geometry, map ENTRIES
  persist — round-trip identity is at RENDER level (`position()`/
  `interior()` readers), not map equality.
- Fixture math (commands tests): 3 boxes 100×50 at (0,0)/(220,60)/
  (450,0), edge10 interiors [(120,40),(200,80)], bounds (-20,-20,
  590,150); distribute gap 125 → box2 220→225; align Right → x 450;
  centre ids[1,3] → +30y; `drag_move((0,0)+(15,5),snap)` → (20,10).

## M2 auto-layout (shipped)

- `layout.rs` (headless, 7 tests): `stored_for` (Filtered → BASE view;
  `content.automatic_layout` — a **ViewContent** field), `config_for`
  (null direction ⇒ LR), `graph_for` = ids + style-resolved sizes only,
  `apply` (placements→top-lefts, route interiors, empty straightens),
  `encode`/`decode` (`"LR|100|50|50|1"`, garbage→None).
- `scene::build(ws, key, scheme, mask, Option<&Overrides>)`; overrides
  REPLACE (elements = TOP-LEFT; relationships = interiors wholesale);
  boundaries/bounds recompute after. DEVIATION (M2-5 row): boundary pad
  stays `BOUNDARY_PADDING = 20`.
- Engine pins: chain (0,0)/(220,0)/(440,0); bench baseline `P3` all
  ~1.00x at close.

## Clippy/lint catalog (cumulative — the gui leg alone catches shell docs)

- egui 0.37: `PointerButton::Primary` (NOT `Left`); no
  `InputState::any_pointer_down` ⇒ `i.pointer.any_down()`;
  `Ui::toggle_value(&mut bool, atoms)` (no `toggle`);
  `Painter::rect_stroke(rect, rounding, stroke, StrokeKind)`;
  `Id::new` DEPRECATED ⇒ track focus via `response.has_focus()` into a
  flag instead; `Context::memory(|m| m.focused())` exists.
- `similar_names` is DENY — `pointer` next to existing `painter` fails
  the gui leg (renamed mine `cursor`).
- Doc line starting `+` ⇒ `doc_lazy_continuation`; `[] as Vec` cast
  ILLEGAL ⇒ `[] as [(f64,f64); 0]` or `Vec::new()`; `float_cmp` allow
  where exactness by design; `manual_midpoint` ⇒ `f64::midpoint`;
  `assigning_clones` ⇒ `clone_into`; slice `.iter().copied().collect()`
  ⇒ `to_vec`; `needless_range_loop`; `many_single_char_names` at >4;
  consts BEFORE statements; guard test bans `assert!(x.is_empty())`;
  benches may `.expect` (e2e.rs precedent); `unused_mut` on closures;
  `cargo fmt` reflows signatures onto one line — match post-fmt text.

## M1 viewer orientation (shipped)

- eframe/egui/epaint 0.37: `App::ui(&mut self, ui, frame)`; unified
  `egui::Panel`; `Storage::set_string(k, String)`; `Window::open(&mut
  bool)` copy-flag-out pattern; `ctx.load_texture`/`Painter::image`;
  epaint `text::Fonts`; `f32: From<u32>` does NOT exist;
  `ComboBox::from_id_salt`, `ui.selectable_value`,
  `DragValue::range(0..=1000)` (i32), `Window::resizable(false)`.
- Click after a DECIDED drag never fires (`could_any_button_be_click`).
- Scene: item rect top-left = element x/y (or override), size = style
  else 350×150; Dn = boundary not item; edge polyline = [source centre,
  vertices…, destination centre]; `hit::pick` items last-wins, edges by
  tolerance, boundaries NOT pickable; `Hit::Item/Edge(usize)` = scene-
  layer indices.
- bless diff-0: `cargo xtask e2e bless 2>&1 | grep -cE ": unchanged$"`
  = 307 + clean `git status` under `testdata/`.
- `cargo test -p oxidizr-viewer` = 65 lib + 3 golden tests.
