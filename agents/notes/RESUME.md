# RESUME — session checkpoint (updated 2026-11-20, **P3-M1 COMPLETE → M2 next**)

## State

- **P2R COMPLETE** `410fee6`; bench baseline `2R` (`6baa7b4`).
  **P3-M0 COMPLETE** (`e3f81e6`→`346ca6e`). **P3-M1 COMPLETE**
  (`969a31b`→`496912c`, log in PROGRESS). **Nothing pushed; tags stay
  v0.1.0/v0.2.0** (v0.3.0 lands at P3-M5).
- **M2 NEXT — `Layered` engine MVP (§P3-D5).** Entry order:
  1. **Write the §P3-M2 decomposition into PHASE_3 FIRST** (charter:
     >4 modules — layout internals, V1 verification, benches/baseline,
     viewer buttons+dialog).
  2. **P3-V1 OPEN**: auto-layout parity INTENT vs monorepo `ad34036`
     (clone under `/tmp/opencode/structurizr-ref`, re-clone per
     `agents/notes/env/toolbox.md` if lost): rank direction per C4
     view type (`automaticLayout` LeftRight/…/TopDown), boundary
     nesting as opaque super-nodes — drives `Layered` tuning; result
     is OUR {x,y}, contract-safe by definition.
  3. `Layered` MVP pipeline (existing `oxidizr-layout` types from
     M0-2 `3775a38`: `LayoutGraph`/`LayoutConfig`/`Anchor`/
     `RankDirection` + `trait LayoutEngine`, ZERO deps): cycle removal
     by DFS reversal → longest-path layering (iteration cap, no
     simplex) → barycenter sweeps w/ deterministic tie-breaks →
     simple priority coordinate pass honoring separations →
     orthogonal edge routing along rank gaps → naive boundary
     contraction (super-nodes, unpack after ordering).
  4. **Determinism is the hard contract**: no randomness, stable
     sorts, fixed iteration counts → property test "same input ⇒
     byte-identical layout" + criterion benches on 1k-node synthetic
     views; P3 JOINS the milestone baseline list (new committed bench
     baseline at close).
  5. Viewer: auto-layout BUTTONS + dialog (direction, generate
     vertices, rank/node separation — `auto-layout.jspf`, feature
     matrix row 12); applied as scene geometry (M3 persists via
     overlay — M2 keeps it session-live unless the plan row says
     otherwise).
  - Quality bar: usable, stable, deterministic C4 layouts — NOT
    Graphviz parity (polish deferred to P4-D13). `Dot` backend is M4
    (`dot-backend` feature, `$OXIDZR_DOT_BIN`).
- **Ritual unchanged:** quartet every commit; API/engine commits also
  full tier (`OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e`, 8 ok) +
  `cargo xtask e2e bless` → 307×unchanged + clean `testdata/`; viewer
  commits also `cargo xtask ci gui`; layout-crate commits run the
  quartet (workspace-wide) — bench `cmp` at M2 close; commit every
  green piece + rewrite this file; **never push**.

## M1 viewer orientation (shipped — read before any viewer work)

- eframe/egui/epaint **0.37 API traps** (all compile-verified):
  `App::ui(&mut self, ui, frame)`; panels unified → `egui::Panel::
  top/bottom/left/right` + `.default_size(f)`; `Ui::scope_id` (not
  `id()`); `Id::unique` (not `Id::new`); `ui.allocate_rect(rect,
  Sense::click_and_drag())`; `ctx.fonts_mut(|f: &mut FontsView| …)`;
  `ctx.open_url(OpenUrl::new_tab(url))`; `Visuals::noninteractive()`
  method; `Storage::set_string(k, String)`; `Window::new(..)
  .open(&mut bool)`; `Painter::rect_stroke(rect, r, stroke,
  StrokeKind)`; `Painter::image(tex_id, rect, uv, tint)` +
  `ctx.load_texture(name, ColorImage, TextureOptions)`; epaint:
  `text::Fonts::new(TextOptions, FontDefinitions)` (NO `FontAtlas`),
  no root `rect()` fn, `Shape::line_segment([Pos2; 2], stroke)`,
  `Align2` via `epaint::emath::Align2`, `Vec2::min_elem`,
  `Pos2::distance`, dashes only via `Shape::dashed_line_many`.
  `f32: From<i32>` AND `f32: From<u32>` do NOT exist (accumulate or
  cast-allow).
- Icon-raster API facts (compile-verified): usvg 0.50
  `Size::width()/height()` return **f32 directly** (no `.get()`);
  `resvg::render(&tree, Transform, &mut pixmap.as_mut())` — NO fontdb
  arg; usvg `Options { resources_dir, ..Default }`, features OFF
  except `svgz`; png 0.18 default output is IDENTITY —
  `set_transformations(ALPHA|EXPAND|STRIP_16)`, `ColorType::
  {Grayscale,Rgb,Rgba}` (no `Gray`). `[a,b,c,d; N]` is NOT array
  syntax — `[u8;4].repeat(n)`; raw string `r#"…"#` breaks on
  `fill="#…"` — `r##"…"##`.
- Shell shape (`main.rs`): `Viewer` = workspace/view/scene/viewport/
  selection + `tag_inventory`/`tag_mode`/`tag_sel`(SESSION)/
  `legend`/`show_diagnostics`/`base_dir`/`icon_texture`/`icon_uv`/
  `icon_pixels`/`icon_errors`/`icons_dirty`; `adopt` derives
  `base_dir` from `self.display` (`-` → None; AFTER the display
  clone — E0382 trap); `sync_icons` runs BEFORE the CentralPanel;
  mask built in `rebuild_scene` only when `tag_sel` non-empty.
  Storage keys `oxidizr.viewer/{theme,zoom,view}` (chips NEVER
  persist, V7).
- `key::session_mask` follows `scene::build` view/base resolution
  exactly (Filtered → base content); hold a function-lifetime `View`
  binding for the content borrow (E0597 trap).
- Guard: 8 lib files (`lib`+`scene`+`hit`+`key`+`icons`+`paint`+
  `commands`+`reload`), forbidden tokens `use eframe`/`eframe::`/
  `use egui`/`egui::` — epaint OK.
- Clippy house traps: doc-list continuations exactly `//!   `;
  `assert!(x.is_empty())` BANNED → `assert_eq!(x, [] as [T; 0])`;
  `too_many_lines` allow must sit on the FN;
  `struct_excessive_bools` allow must sit on the STRUCT (field =
  no-op); `assigning_clones` → `src.clone_into(&mut dst)`;
  `f32::from(f32_const)` = useless_conversion; nursery
  `format_push_string` → `write!`; per-fn
  `#[allow(clippy::cast_precision_loss)] // exact small indices`;
  `missing_errors_doc` needs `# Errors` on pub Result fns;
  `redundant_clone` branch fix: clone into the set, MOVE the key into
  the row when the branch ends in `continue`.
- Core view structs have NO `Default` (Model/ViewContent DO);
  `SoftwareSystem` has NO `aliases` field. Tests: `#![allow(clippy::
  unwrap_used)]`; integration fixture dirs must be PER TEST (parallel
  race). Float asserts only exactly-representable values.
- bless diff-0 check: `cargo xtask e2e bless 2>&1 | grep -cE ":
  unchanged$"` = 307 + clean `git status` under `testdata/`.
