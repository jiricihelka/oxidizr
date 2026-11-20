# RESUME — session checkpoint (updated 2026-11-20, **P3 M2-2 done → M2-3 next**)

## State

- **P2R COMPLETE** `410fee6`; **P3-M0** (`e3f81e6`→`346ca6e`);
  **P3-M1 COMPLETE** (`969a31b`→`496912c`); §P3-M2 decomposition +
  V1 CLOSED `c626048`. **Nothing pushed; tags stay v0.1.0/v0.2.0**
  (v0.3.0 lands at P3-M5).
- **M2 (`Layered`, §P3-M2 table)** — progress:
  - **M2-1 DONE** `c626048` (V1 verdict + decomposition).
  - **M2-2 DONE 2026-11-20** (this commit): `crates/oxidizr-layout/
    src/layered.rs` — `Layered` engine landed **complete end-to-end**
    (deviation recorded in the PHASE_3 row): DFS cycle reversal →
    Kahn longest-path ranks → dummy chains → naive stacked coords →
    anchor-aware routes; `impl LayoutEngine for Layered` +
    `LayoutError::CycleRemovalFailed` (defensive-only). 21 unit tests
    (exact-float coords), clippy clean.
  - **M2-3 NEXT**: REPLACE the naive stage-4 order + stage-5 coords —
    barycenter sweeps (FIXED count, deterministic tie-breaks) +
    priority coordinate pass + orthogonal-ish routing along rank gaps;
    determinism property test up to 1k-node synthetic. Existing tests
    pin today's naive coords — expect exact-float updates where the
    new passes change positions (update TESTS, they encode the new
    contract; do not weaken assertions).
  - **M2-4**: criterion benches in `oxidizr-layout` (`[[bench]]
    harness = false` per the `reconcile` pattern, 1k-node synthetic).
  - **M2-5**: viewer headless `layout` module (scene→`LayoutGraph`,
    `Overrides`), `scene::build` gains `Option<&Overrides>`,
    auto-layout button + dialog (`auto-layout.jspf` mirror), ONE
    pass at open for views with stored `automaticLayout`; boundaries
    = member-union rects + 50 px pad (V1). Gates incl. `ci gui`.
  - **M2 close**: PROGRESS/PHASE_3/RESUME + `bench save P3` +
    `bench cmp P3`. Then M3 (V4/V6), M4 (V8, `dot-backend`), M5
    (oracle verify + tag v0.3.0).
- **Ritual:** quartet every commit; engine/API commits also full tier
  (`OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e` → 8 ok) + `cargo
  xtask e2e bless` → 307×unchanged + clean `testdata/`; viewer commits
  also `cargo xtask ci gui`; **never push**. M2-2 gates all green
  2026-11-20.

## M2 layout-engine orientation (read first)

- `layered.rs` design facts: **canonical frame** = rank axis → x,
  cross axis → y; TopDown/BottomUp swap, RightLeft/BottomUp mirror
  via the `along` rank extent (`map_box` subtracts the node extent
  when mirroring, `map_point` plain-flips — points have no extent);
  canonical rank extent = WIDTH horizontal / HEIGHT vertical.
  Ranks stack with `node_separation`, centred on a shared mid-line.
- Routes (stage 6, stays through M2-3): anchor side-point (explicit
  `Anchor` or dominant-delta side facing the neighbor, exact ties
  horizontal) → dummy centres (only when `generate_vertices`, walked
  in INPUT direction over the acyclic chain, reversed chains run
  backwards) → anchor. Self-loop = two stubs on the box right side
  (y + h·0.25 / 0.75).
- Exact-float test fixtures: 120×60 nodes, default config
  (LR/100/50/50/true): chain = (0,0),(220,0),(440,0); diamond b/c at
  y 0/110, a/d y 55; long-edge a→c dummy waypoint (280,110); TD pair
  b at (0,160). All exactly representable — keep it that way.
- Clippy traps hit here: `many_single_char_names` fires at **>4**
  single-char bindings in scope (rename tuple destructurings:
  `bx/by/bw/bh`, `ri`/`vid`, `vert`/`pt`); `items_after_statements`
  → consts BEFORE all statements incl. the first `let`;
  `too_many_lines` allow on the FN; `redundant_clone` → move the key
  when the branch ends in `continue`; `missing_errors_doc` needs
  `# Errors` on pub Result fns.
- Patch-script habit that bit twice: python `rep()` scripts write
  only at the END — one missed assert loses ALL edits. Read the
  current post-fmt text FIRST, then patch.

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
  `use egui`/`egui::` — epaint OK; viewer lib must ALSO stay free of
  the layout crate's GUI-ness (layout crate itself is zero-dep — the
  viewer depends on it, not vice versa).
- Clippy house traps: doc-list continuations exactly `//!   `;
  `assert!(x.is_empty())` BANNED → `assert_eq!(x, [] as [T; 0])`;
  `struct_excessive_bools` allow on the STRUCT; `assigning_clones` →
  `src.clone_into(&mut dst)`; `f32::from(f32_const)` =
  useless_conversion; nursery `format_push_string` → `write!`.
- Core view structs have NO `Default` (Model/ViewContent DO).
  Tests: `#![allow(clippy::unwrap_used)]`; fixture dirs PER TEST.
  Float asserts only exactly-representable values.
- bless diff-0 check: `cargo xtask e2e bless 2>&1 | grep -cE ":
  unchanged$"` = 307 + clean `git status` under `testdata/`.
