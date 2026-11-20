# RESUME — session checkpoint (updated 2026-11-20, **P3 M3-2 done → M3-3 shell next**)

## State

- **P2R COMPLETE** `410fee6`; **P3-M0** (`e3f81e6`→`346ca6e`);
  **P3-M1 COMPLETE** (`969a31b`→`496912c`); **P3-M2 COMPLETE**
  (`c626048`→`e647dcd`, close `468ec03` + `P3` bench baseline);
  **P3-M3 STARTED** — M3-1 = §P3-M3 decomposition + V4 CLOSED
  (this commit). **Nothing pushed; tags v0.1.0/v0.2.0** (v0.3.0 at
  P3-M5).
- **Ritual:** quartet every commit; shell commits also `cargo xtask ci
  gui`; lib commits also full tier (`OXIDZR_E2E_FULL=1 cargo test
  -p oxidizr-e2e` → 8 ok) + `cargo xtask e2e bless` → 307×unchanged +
  clean `testdata/`; **never push**.
- **M3 plan (§P3-M3 table in PHASE_3.md — authoritative, read it):**
  - **M3-2 DONE (this commit)**: headless `commands` —
    `LayoutOverlay { view_key, elements, relationships, dimensions }`
    (`for_view`, `is_empty`, `as_overrides`, `position`/`rect`/
    `interior` read overlay-first-then-scene); `Command`
    (MoveElements{moves}/SetWaypoints{changes}/SetCanvasSize{from,to}/
    ClearView{snapshot}) with symmetric `apply(forward)` — `from`/`to`
    travel in the command, no closures; factories `move_by`(snap flag,
    GRID_PX 10), `align`(AlignEdge l/c/r/t/m/b, bbox target),
    `distribute`(Axis, needs ≥3, extremes fixed, equal gaps incl.
    negative), `centre_view`(bounds centre), `bring_back`(page Rect,
    minimal delta), `remove_all_vertices`, `set_waypoints`,
    `set_canvas_size`, `clear_view` — ALL return `Option<Command>`
    (None = no-op, never pushed); `History` run/undo/redo/reset/
    can_*, HISTORY_CAP 200 (drop-oldest, run clears redo); type
    aliases `ElementMove`/`WaypointChange`; 8 unit tests (round-trip
    per variant, cap, fixture-factory math). Viewer lib now 62 tests.
  - **M3-3 NEXT** shell: drag-move + box-select + pan→middle/space +
    Ctrl+Z/Y keys + per-view overlay map (replaces
    `Viewer.layout_overrides`) + dirty badge + Apply/Reset as commands;
    add `hit::items_in_rect(scene, Rect) -> Vec<Id>` headless helper
    (content order, INTERSECTS semantics) for box-select.
  - **M3-4** waypoint handles (drag/insert/Alt-delete) + dialog footer
    buttons (magnet bring-back, remove-vertices) + align/distribute row.
  - **M3-5** SAVE: `apply_overlay` (clone workspace, write x/y +
    vertices + dimensions) → `oxidizr_json::to_vec` → temp+rename;
    Ctrl+S; `.json` in place, DSL → `<stem>.json` sidecar; filtered
    views save-blocked; Category A round-trip test.
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

## M2 auto-layout (shipped — read before M3)

- `crates/oxidizr-viewer/src/layout.rs` (headless, guard-covered, 7
  tests): `stored_for(ws, view_key)` (Filtered → BASE view; reads
  `content.automatic_layout` — it's a **ViewContent** field, NOT
  ViewBase), `config_for(&AutomaticLayout)` (null direction ⇒ LR;
  `direction()` maps TopBottom→TopDown etc.), `graph_for(scene, config)`
  = ids + **style-resolved sizes only** (idempotent re-runs!), `apply`
  (placements→top-lefts, route `points[1..len-1]`→interior waypoints,
  empty vec straightens), `encode`/`decode` (`"LR|100|50|50|1"`,
  garbage→None).
- `scene::build(ws, key, scheme, mask, Option<&Overrides>)` — 5th param;
  `Overrides { elements: HashMap<Id,(f64,f64)>, relationships:
  HashMap<Id,Vec<(f64,f64)>> }`: element TOP-LEFT replaces stored x/y;
  relationship interiors REPLACE stored vertices (empty = straight);
  boundaries/bounds recompute after (group test pins it). DEVIATION
  recorded on the M2-5 row: boundary pad stays `BOUNDARY_PADDING = 20`.
- Shell: `Viewer.{layout_overrides, layout_error, show_layout,
  layout_params}`; adopt/view-select: clear overrides → rebuild →
  `autolayout_once()` (ONE pass iff view carries stored automaticLayout);
  `run_layout` = apply→overrides→rebuild→fit, errors→`layout_error`
  (status bar `.or()` chain + dialog red); `layout_window` dialog
  (ComboBox+DragValues+checkbox, Apply/Reset); `Layout` toolbar button;
  KEY_LAYOUT persists PARAMS only.
- Engine (`oxidizr-layout`): DFS cycle reversal → longest-path ranks →
  dummies → `BARYCENTER_ROUNDS=4` cascading sweeps (best-of by
  exhaustive crossings, ties keep EARLIER) → `PRIORITY_ROUNDS=4`
  alternating-side passes (dummies first, clamp to within-rank slots,
  over-constrained→high, min-REAL b→0) → orthogonal Z-channels +
  `collapse`. Exact pins: 350×150 chain (0,0)/(450,0)/(900,0); 120×60
  chain (0,0)/(220,0)/(440,0); long-edge a(0,40)/b(220,0)/c(440,40) +
  6-pt route. Benches `p3_layered_thousand_{mixed,chain,star}` ≈
  3.7/0.71/0.97 ms; `bench cmp P3` at close = all ~1.00x (fresh
  baseline).

## Clippy/lint catalog (cumulative — the gui leg alone catches shell docs)

- Doc line starting `+` ⇒ `doc_lazy_continuation` (main.rs caught it at
  M2-5); `[] as Vec` cast ILLEGAL ⇒ `[] as [(f64,f64); 0]` or
  `Vec::new()`; `float_cmp` allow where exactness by design;
  `manual_midpoint` ⇒ `f64::midpoint`; `assigning_clones` ⇒
  `clone_into`; slice `.iter().copied().collect()` ⇒ `to_vec`;
  `needless_range_loop`; `many_single_char_names` at >4; consts BEFORE
  statements; guard test bans `assert!(x.is_empty())`; benches may
  `.expect` (e2e.rs precedent); `unused_mut` on closures.

## M1 viewer orientation (shipped)

- eframe/egui/epaint 0.37: `App::ui(&mut self, ui, frame)`; unified
  `egui::Panel`; `Storage::set_string(k, String)`; `Window::open(&mut
  bool)` copy-flag-out pattern; `ctx.load_texture`/`Painter::image`;
  epaint `text::Fonts`; `f32: From<u32>` does NOT exist. Dialog APIs
  verified: `ComboBox::from_id_salt`, `ui.selectable_value`,
  `DragValue::range(0..=1000)` (i32), `Window::resizable(false)`.
- Scene: item rect top-left = element x/y (or override), size = style
  else 350×150; Dn = boundary not item; edge polyline = [source centre,
  vertices…, destination centre]; `Rect::union`/`expand(pad)`; legend
  from scene; boundary guard = 9 lib files (eframe/egui banned, epaint
  OK). Shell keys `oxidizr.viewer/{theme,zoom,view,layout}`; `adopt()`
  derives base_dir AFTER display clone (E0382).
- bless diff-0: `cargo xtask e2e bless 2>&1 | grep -cE ": unchanged$"`
  = 307 + clean `git status` under `testdata/`.
- `cargo test -p oxidizr-viewer` = 54 lib + 3 golden tests.
