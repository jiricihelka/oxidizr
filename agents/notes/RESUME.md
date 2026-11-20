# RESUME — session checkpoint (updated 2026-11-20, **P3 M2-3 done → M2-4 next**)

## State

- **P2R COMPLETE** `410fee6`; **P3-M0** (`e3f81e6`→`346ca6e`);
  **P3-M1 COMPLETE** (`969a31b`→`496912c`); M2 pre-work `c626048`;
  **M2-2** `94a8d82`; **M2-3 DONE 2026-11-20** (this commit).
  **Nothing pushed; tags v0.1.0/v0.2.0** (v0.3.0 at P3-M5).
- **M2 (`Layered`, §P3-M2 table)** — progress:
  - **M2-1/2/3 DONE**: `oxidizr-layout::Layered` is the full
    pipeline — DFS cycle reversal → longest-path ranks → dummy
    chains → **barycenter sweeps** → **priority coordinate passes**
    → **orthogonal Z-channel routes**. 24 unit tests green.
  - **M2-4 NEXT — benches**: criterion targets in
    `crates/oxidizr-layout/benches/` (`[[bench]] harness = false` +
    `criterion = { workspace = true }`-style deps in `[dev-dependencies]`,
    per the `reconcile` pattern in `crates/oxidizr-cli/Cargo.toml`
    — copy its exact shape). Shapes: 1k-node synthetic (reuse the
    `thousand_nodes_are_deterministic` generator pattern: mixed
    fan/back-edges), a long-chain, a star/fan; bench `Layered.layout`
    whole-pipeline. The bench dev-dep does NOT break the zero-dep
    promise (dev-deps are not deps — record in the Cargo.toml comment
    if in doubt). Gate: quartet + a `cargo bench -p oxidizr-layout`
    smoke run.
  - **M2-5**: viewer integration: headless `layout` module
    (scene→`LayoutGraph`, engine run, `Overrides { element rects,
    edge waypoints }`), `scene::build` gains `Option<&Overrides>`
    (session-live, NEVER persisted), auto-layout button + dialog
    (`auto-layout.jspf` mirror: direction, vertices, rank/node/edge
    separation), ONE pass at open for views with stored
    `automaticLayout`; boundaries follow via member-union rects +
    50 px pad (V1). Gates incl. `ci gui`.
  - **M2 close**: PROGRESS/PHASE_3/RESUME + `cargo xtask bench save
    P3` + `bench cmp P3`. Then M3 (V4/V6), M4 (V8, `dot-backend`),
    M5 (oracle verify + tag v0.3.0).
- **Ritual:** quartet every commit; engine/API commits also full tier
  (`OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e` → 8 ok) + `cargo
  xtask e2e bless` → 307×unchanged + clean `testdata/`; viewer
  commits also `cargo xtask ci gui`; **never push**. M2-3 gates all
  green 2026-11-20.

## M2 layout-engine orientation (read first)

- `layered.rs` pipeline facts (M2-3 state):
  - Ordering (`Skeleton::order/sweep` + free fn `crossings`):
    `BARYCENTER_ROUNDS = 4`, cascading sweeps (positions live-update
    per swept rank), empty-neighbour side ⇒ barycentre = current
    position; best order = min crossings, ties keep EARLIER order.
  - Coords (`Skeleton::priority`): `PRIORITY_ROUNDS = 4` alternating
    preds/succs; desired = mean neighbour CENTRE (snapshot `start`);
    dummies placed first per rank (prio 1), reals by slot order;
    clamp interval from within-rank neighbours (placed ⇒ LIVE centre,
    unplaced ⇒ snapshot centre — order+separations preserved);
    `target = desired.max(low).min(high)` — over-constrained resolves
    to `high`; final shift: min REAL `b` → 0.
  - Routes: spine = anchor → dummy centres → anchor, INPUT direction
    over the acyclic chain; legs axis-parallel direct, diagonals →
    Z at mid-RANK coord (rank-gap channel), or mid-CROSS coord when
    both ends pin Cross sides (`SideKind` from `endpoint`);
    `collapse` drops duplicates + collinear middles;
    `generate_vertices=false` ⇒ straight 2-point route; self-loop =
    2 stubs at right side (y+h·0.25/0.75). Use `f64::midpoint` (not
    `(a+b)/2` — `manual_midpoint` nursery lint).
- Exact-float pins (120×60 nodes, LR/100/50/50/true): chain
  (0,0)/(220,0)/(440,0); diamond a/d y 55, b/c y 0/110 (UNCHANGED by
  priority); long-edge a (0,40), b (220,0), c (440,40), ac route
  120,70→200,70→200,110→360,110→360,70→440,70 (a/c centre 70 =
  mean(b 30, dummy 110)); cycle `ca` = mirror of that +120 x-offset
  pattern; anchors Top/Bottom = cross-Z at y 30 (4 pts).
- Clippy traps (M2 additions): doc continuation line starting `+` =
  `doc_list_item_without_indentation` (reword); `manual_midpoint`;
  `needless_range_loop` (iterate `.skip(1)` not index);
  `explicit_iter_loop`; `assigning_clones` → `src.clone_into(&mut
  dst)`; `iter().copied().collect()` on a slice → `to_vec()`;
  `float_cmp` allow on fns comparing exact layout values (+ the one
  test asserting axis-alignment); `many_single_char_names` at >4
  single-char bindings; `items_after_statements` consts before ALL
  statements; `too_many_lines` allow on the FN.
- Patch-script habit: read the CURRENT post-fmt text before writing
  `rep()` patches — fmt reflows multi-line fn signatures.

## M1 viewer orientation (shipped — read before M2-5)

- eframe/egui/epaint **0.37 API traps** (compile-verified):
  `App::ui(&mut self, ui, frame)`; `egui::Panel::top/bottom/left/
  right` + `.default_size(f)`; `Ui::scope_id`; `Id::unique`;
  `ui.allocate_rect(rect, Sense::click_and_drag())`; `ctx.fonts_mut
  (|f: &mut FontsView| …)`; `ctx.open_url(OpenUrl::new_tab(url))`;
  `Visuals::noninteractive()`; `Storage::set_string(k, String)`;
  `Window::new(..).open(&mut bool)`; `Painter::rect_stroke(rect, r,
  stroke, StrokeKind)`; `Painter::image(tex_id, rect, uv, tint)` +
  `ctx.load_texture(name, ColorImage, TextureOptions)`; epaint:
  `text::Fonts::new(TextOptions, FontDefinitions)`, `Shape::
  line_segment([Pos2; 2], stroke)`, `Align2` via `epaint::emath::
  Align2`, `Vec2::min_elem`, `Pos2::distance`, dashes via
  `Shape::dashed_line_many`. `f32: From<u32>` does NOT exist.
- Icons: usvg 0.50 `Size::width()/height()` → f32 directly;
  `resvg::render(&tree, Transform, &mut pixmap.as_mut())` NO fontdb;
  zero default features → SVG-with-text renders text-less; png 0.18
  `set_transformations(ALPHA|EXPAND|STRIP_16)`.
- Shell (`main.rs`): `Viewer` = workspace/view/scene/viewport/
  selection + tag/legend/diagnostics/icon fields; `adopt` derives
  `base_dir` from `self.display` AFTER the display clone (E0382);
  `sync_icons` before the CentralPanel; storage keys
  `oxidizr.viewer/{theme,zoom,view}`; chips never persist (V7).
- Scene seam for M2-5: item rects built from `element.x/y` at
  scene.rs ~487; boundaries = member-union `Rect::union` + pad;
  `scene::build(ws, view, scheme, mask)` — `Overrides` param goes
  onto this signature.
- Guard: 8 lib files, forbidden tokens `use eframe`/`eframe::`/
  `use egui`/`egui::` — epaint OK (a new `layout` module joins the
  guard list: 9 files).
- Clippy house traps: doc-list continuations exactly `//!   `;
  `assert!(x.is_empty())` BANNED → `assert_eq!(x, [] as [T; 0])`;
  `struct_excessive_bools` allow on the STRUCT; `f32::from(
  f32_const)` = useless_conversion; nursery `format_push_string` →
  `write!`; `missing_errors_doc` needs `# Errors` on pub Result fns.
- Core view structs have NO `Default` (Model/ViewContent DO).
  Tests: `#![allow(clippy::unwrap_used)]`; fixture dirs PER TEST.
  Float asserts only exactly-representable values.
- bless diff-0 check: `cargo xtask e2e bless 2>&1 | grep -cE ":
  unchanged$"` = 307 + clean `git status` under `testdata/`.
