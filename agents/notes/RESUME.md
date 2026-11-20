# RESUME — session checkpoint (updated 2026-11-20, **P3-M1-5 done → M1 close next**)

## State

- **P2R COMPLETE** `410fee6`; bench baseline `2R` (`6baa7b4`).
  **P3-M0 COMPLETE** (`e3f81e6`→`346ca6e`). **Nothing pushed; tags stay
  v0.1.0/v0.2.0** (v0.3.0 lands at P3-M5).
- **P3-M1 (read-only viewer)** — decomposition §P3-M1 in
  `agents/plan/build/PHASE_3.md`. Progress:
  - **M1-1 DONE** `969a31b`: `oxidizr-api::open()` partial-DSL open.
  - **M1-2 DONE** `28fd3bf`: `scene.rs` headless scene core (14 tests).
  - **M1-3 DONE** `c85bf1c`: `hit.rs` viewport + pick (10 tests).
  - **M1-4a DONE** `36c36c1`: `paint.rs` headless paint + gui shell
    (open/navigate/select/tooltips/theme/persistence).
  - **M1-4b DONE** `3662671`: headless `key` module — `legend(&Scene)`
    (deduped rows), `tags(&Workspace)` inventory, `session_mask` →
    `filter::eval` on the RENDER BASE (V5/V7, session-only); chips +
    key panel + diagnostics window. Include trap: a relationship
    needs ITS OWN tags to hit.
  - **M1-5 DONE 2026-11-20**: headless `icons` module — `rasterize`
    (data URIs base64/percent, file paths vs `base_dir`, `http(s)`
    NEVER fetched, foreign bytes `Unsupported`) + `build_atlas`
    (deterministic shelf pack, keyed icon×size×scheme,
    `ICON_RASTER_PX` 64, failures once each in `Atlas::errors`).
    **Raster seam: `resvg` 0.50** (usvg 0.50/tiny-skia 0.12 trio;
    tiny-skia 0.12 removed the 0.11 usvg `Renderer`) — usvg/resvg
    ZERO default features (no fontdb) → SVG-with-text renders
    text-less (§P3-D7, V3 moots it). Shell: ONE texture upload, cells
    as ½-px-inset uv rects overlaid after `paint_scene` via pub
    `paint::icon_slot`/`to_screen_rect`; placeholder stays for failed
    icons; key-row chips; diagnostics + status count include icon
    errors; `ElementKey.icon` quintuple; guard 8 lib files; 8 unit
    tests.
  - **M1 close NEXT**: PROGRESS board + log M1 row; PHASE_3 §P3-M1
    header → M1 DONE; this file → M2 entry point. Full ritual (docs
    only — bench N/A, engine untouched, note it).
- **M2 entry point (Layered, V1 OPEN)**: §P3-M2 in PHASE_3 —
  `oxidizr-layout::Layered` against `LayoutGraph` (M0-2 types,
  zero-dep crate); V1 = parity INTENT only (our {x,y}, contract-safe
  by definition); viewer applies results as scene overrides.
- **P3-V items**: V2/V3/V5/V7 CLOSED; open: V1 (M2), V4+V6 (M3),
  V8 (M4). Reference clone `ad34036` under `/tmp/opencode/structurizr-ref`
  — re-clone per `agents/notes/env/toolbox.md` if lost.
- **Ritual unchanged:** quartet every commit; API/engine commits also
  full tier (`OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e`, 8 ok) +
  `cargo xtask e2e bless` → 307×unchanged; viewer commits also
  `cargo xtask ci gui`; commit every green piece + rewrite this file;
  **never push**.

## M1 orientation (read first)

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
- Icon-raster API facts (compile-verified 2026-11-20): usvg 0.50
  `Size::width()/height()` return **f32 directly** (no `.get()`);
  `resvg::render(&tree, Transform, &mut pixmap.as_mut())` — NO fontdb
  arg; usvg `Options { resources_dir, ..Default }` with
  `default-features = false, features = ["svgz"]`; png 0.18 default
  output is IDENTITY — `set_transformations(ALPHA|EXPAND|STRIP_16)`,
  `ColorType::{Grayscale,Rgb,Rgba}` (no `Gray`); strict base64
  (`"R0lGODlh"` = GIF89a magic, padding-free). `[a,b,c,d; N]` repeat
  is NOT array syntax — use `[u8;4].repeat(n)`. Raw string `r#"…"#`
  breaks on `fill="#…"` payloads — use `r##"…"##`.
- Shell shape (`main.rs`): `Viewer` fields += `tag_inventory`/
  `tag_mode(FilterMode)`/`tag_sel`(SESSION)/`legend(key::Key)`/
  `show_diagnostics`/`base_dir`/`icon_texture`/`icon_uv`/
  `icon_pixels`/`icon_errors`/`icons_dirty`; mask built in
  `rebuild_scene` only when `tag_sel` non-empty; `adopt` recomputes
  inventory + clears chips + derives `base_dir` from `self.display`
  (`-` → None; AFTER the display clone — E0382 trap); `sync_icons`
  runs BEFORE the CentralPanel. Storage keys `oxidizr.viewer/
  {theme,zoom,view}` (chips NEVER persist, V7).
- `key::session_mask` follows the `scene::build` view/base resolution
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
