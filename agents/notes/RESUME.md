# RESUME — session checkpoint (updated 2026-11-20, **P3 M2 COMPLETE → M3 next**)

## State

- **P2R COMPLETE** `410fee6`; **P3-M0** (`e3f81e6`→`346ca6e`);
  **P3-M1 COMPLETE** (`969a31b`→`496912c`); **P3-M2 COMPLETE** —
  **M2-1** `c626048`, **M2-2** `94a8d82`, **M2-3** `344683b`,
  **M2-4** `98a4d60`, **M2-5** `e647dcd`, close = docs + `bench save
  P3` baseline (this commit). **Nothing pushed; tags v0.1.0/v0.2.0**
  (v0.3.0 at P3-M5).
- **Ritual:** quartet every commit; viewer commits also `cargo xtask ci
  gui`; engine/API commits also full tier (`OXIDZR_E2E_FULL=1 cargo test
  -p oxidizr-e2e` → 8 ok) + `cargo xtask e2e bless` → 307×unchanged +
  clean `testdata/`; **never push**.
- **NEXT: M3 editing + save (§P3-D4, P3-D9 M3 bullet).** Charter first:
  M3 touches >4 modules (commands, scene, shell, save, tests) ⇒ write a
  **§P3-M3 decomposition table** in PHASE_3 as the M3-1 docs commit
  (M2-1 pattern), checking §P3-D10 **V4** (dialog/auto-layout
  semantics) + **V6** (save/write-back) rows against the monorepo
  BEFORE coding. Scope from §P3-D4: `LayoutOverlay` (commands.rs stub —
  id → geometry deltas over the read-only workspace), invertible
  commands (`MoveElement`, `SetWaypoints`/`Insert`/`Delete`,
  `SetAnchor`, `ClearViewLayout`, align l/c/r/t/m/b + distribute h/v,
  `CenterViewOrSelection`, `SetCanvasSize` → stored dimensions,
  `BringOffCanvasBack`, `RemoveAllEdgeWaypoints`), Vec undo/redo cap
  200 (Ctrl+Z/Ctrl+Shift+Z/Ctrl+Y), drag + grid snap 10 px, box
  select, waypoint drag/insert/delete; **SAVE = clone workspace →
  apply overlay (elementViews x/y, relationshipViews vertices) →
  `oxidizr-json` writer (WRITE-CATALOG, NO new serializer) → Ctrl+S to
  `<name>.json` sidecar** — byte-compat gate: oracle round-trip check.
  The M2 `Overrides` seam is the geometry substrate: overlay ≈ stored
  + overrides merge; `relationships[id] = []` already IS
  "remove vertices". reload.rs stays a stub until M4 (with minimap +
  export + dot-backend; M4 also gets live reload per P3-D6).
- **M4** (P3-D9): live reload + remap UX, sidecar workflow, minimap +
  quick-nav, export SVG+PNG, `dot-backend` feature (`$OXIDZR_DOT_BIN`).
- **M5**: oracle verify (needs `$OXIDZR_STRUCTURIZR_BIN`), perf pass,
  docs, tag **v0.3.0**.

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
