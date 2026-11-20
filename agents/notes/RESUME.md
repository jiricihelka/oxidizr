# RESUME — session checkpoint (updated 2026-11-20, **P3 M2-5 done → M2 close next**)

## State

- **P2R COMPLETE** `410fee6`; **P3-M0** (`e3f81e6`→`346ca6e`);
  **P3-M1 COMPLETE** (`969a31b`→`496912c`); M2: **M2-1** `c626048`,
  **M2-2** `94a8d82`, **M2-3** `344683b`, **M2-4** `98a4d60`,
  **M2-5 DONE 2026-11-20** (this commit). **Nothing pushed; tags
  v0.1.0/v0.2.0** (v0.3.0 at P3-M5).
- **M2 close NEXT** (§P3-M2 "M2 close" row): PROGRESS board + log M2
  row; PHASE_3 M2 row → DONE; `cargo xtask bench save P3` +
  `cargo xtask bench cmp P3` sanity (three benches:
  `p3_layered_thousand_{mixed,chain,star}`, measured 3.7/0.71/0.97 ms);
  RESUME → M3. Then:
  - **M3** (V4/V6): editing + reload — drag moves, "bring elements
    back"/"remove vertices" commands, SAVE incl. layout persistence
    (overrides become storable `x/y` + vertices — the Overrides→
    ViewContent write-back; `commands`/`reload` modules are M1 seeds),
    edit-undo? check PHASE_3 §P3-M3 row first.
  - **M4** (V8): export PNG/SVG + `dot-backend` feature (`$OXIDZR_DOT_BIN`).
  - **M5**: `cargo xtask oracle verify` (needs `$OXIDZR_STRUCTURIZR_BIN`),
    manual QA, docs, tag **v0.3.0**.
- **Ritual:** quartet every commit; viewer commits also `cargo xtask ci
  gui`; engine/API commits also full tier (`OXIDZR_E2E_FULL=1 cargo test
  -p oxidizr-e2e` → 8 ok) + `cargo xtask e2e bless` → 307×unchanged +
  clean `testdata/`; **never push**. M2-5 gates ALL green 2026-11-20.

## M2-5 auto-layout integration (shipped — read before M3)

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
- eframe 0.37 dialog APIs VERIFIED compiling: `ComboBox::from_id_salt`,
  `ui.selectable_value`, `DragValue::range(0..=1000)` (i32 OK),
  `Window::resizable(false)`, `Grid::new(id).num_columns(2).spacing`.

## M2 layout-engine orientation (shipped)

- `layered.rs` pipeline: DFS cycle reversal → longest-path ranks →
  dummy chains → `BARYCENTER_ROUNDS=4` cascading sweeps (best-of by
  exhaustive crossings, ties keep EARLIER) → `PRIORITY_ROUNDS=4`
  alternating-side passes (dummies first, clamp from within-rank
  neighbours, over-constrained→high, min-REAL b→0) → orthogonal
  Z-channels (mid-RANK coord; mid-CROSS when both ends pin Cross
  sides) + `collapse`. `generate_vertices=false` ⇒ straight 2-pt route;
  self-loop = 2 right-side stubs.
- Exact-float pins: 350×150 boxes chain = (0,0)/(450,0)/(900,0)
  (layout.rs); 120×60: chain (0,0)/(220,0)/(440,0); long-edge a(0,40)/
  b(220,0)/c(440,40) + 6-pt route (x200/x360 channels, y70→110).
- Clippy traps (cumulative): doc line starting `+` ⇒ doc_lazy_continuation
  (gui leg only catches shell ones!); `manual_midpoint` → `f64::midpoint`;
  `float_cmp` allow where exactness by design; `assigning_clones` →
  `clone_into`; slice `.iter().copied().collect()` → `to_vec`;
  `needless_range_loop`; `many_single_char_names` at >4; consts BEFORE
  statements; `[] as Vec` cast ILLEGAL → `[] as [(f64,f64); 0]` or
  `Vec::new()`; benches may `.expect` (e2e.rs precedent).

## M1 viewer orientation (shipped)

- eframe/egui/epaint 0.37: `App::ui(&mut self, ui, frame)`; panels +
  `.default_size`; `Storage::set_string(k, String)`; `Window::open(
  &mut bool)` (copy-flag-out pattern: `let mut open = true; …; self.
  show_x = open;`); `ctx.load_texture`/`Painter::image`; epaint text
  via `text::Fonts`; `f32: From<u32>` does NOT exist.
- Scene: item rect top-left = element x/y (or override), size = style
  width/height else 350×150; Dn = boundary not item; edge polyline =
  [source centre, vertices…, destination centre]; `Rect::union`/
  `expand(pad)`; legend from scene.
- Shell keys `oxidizr.viewer/{theme,zoom,view,layout}`; chips/tags
  never persist geometry; `adopt()` derives base_dir AFTER display
  clone (E0382).
- bless diff-0: `cargo xtask e2e bless 2>&1 | grep -cE ": unchanged$"`
  = 307 + clean `git status` under `testdata/`.
