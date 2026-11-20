# RESUME — session checkpoint (updated 2026-11-20, **P3-M1 in progress → M1-4b next**)

## State

- **P2R COMPLETE** `410fee6`; bench baseline `2R` (`6baa7b4`).
  **P3-M0 COMPLETE** (`e3f81e6`→`346ca6e`). **Nothing pushed; tags stay
  v0.1.0/v0.2.0** (v0.3.0 lands at P3-M5).
- **P3-M1 (read-only viewer)** — decomposition §P3-M1 in
  `agents/plan/build/PHASE_3.md`. Progress:
  - **M1-1 DONE** `969a31b`: `oxidizr-api::open()` partial-DSL open.
  - **M1-2 DONE** `28fd3bf`: `scene.rs` headless scene core, 14 tests
    (text = plain String runs — epaint deferred to M1-4, recorded).
  - **M1-3 DONE** `c85bf1c`: `hit.rs` viewport + pick, 10 tests
    (clamps 0.05/20 NaN-safe, tolerance 6 px ÷ scale, pivot zoom).
  - **M1-4a DONE 2026-11-20** (a/b split recorded on the §P3-M1 row):
    `src/paint.rs` headless paint (`paint_scene → Vec<epaint::Shape>`,
    7 unit tests) + `main.rs` shell rewrite (open argv/toolbar, view
    list, pan/wheel-zoom/select, ctrl-click URL, tooltips, Fit,
    light/dark + `dark-light` startup, eframe-storage theme/zoom/view,
    status bar + diagnostics-hover). `epaint` = unconditional lib dep
    (guard now asserts 6 lib files, still eframe/egui-only); `clap`
    behind `gui`. `tests/golden_paint.rs` = 3 Category-A open→build→
    paint tests on real DSL text (caught + fixed LEFT→CENTER_TOP box
    text anchoring; fixture dirs must be per-test, parallel tests race
    on shared temp paths).
  - **M1-4b NEXT**: key/legend panel, tag-filter chips (include/exclude
    → `filter::eval` → `Included` mask → scene rebuild, session-only
    per V7), diagnostics panel (full log list; hover-preview exists).
    Gate `cargo xtask ci gui` too. Then M1-5 icons (usvg 0.50 +
    tiny-skia 0.12 atlas per (icon,size,scheme), placeholder = dashed
    box already painted by `paint::icon_placeholder`; missing icon →
    warning diagnostic), then the M1-close commit.
- **P3-V items**: V2/V3/V5/V7 CLOSED; open: V1 (M2 Layered), V4+V6
  (M3), V8 (M4). Reference clone `ad34036` under
  `/tmp/opencode/structurizr-ref` — re-clone per
  `agents/notes/env/toolbox.md` if lost.
- **Ritual unchanged:** quartet every commit; API/engine commits also
  full tier (`OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e`, 8 ok) +
  `cargo xtask e2e bless` → 307×unchanged; viewer commits also
  `cargo xtask ci gui`; commit every green piece + rewrite this file;
  **never push**.

## M1 orientation (read first)

- eframe/egui/epaint **0.37 API traps** (all compile-verified):
  `App::ui(&mut self, ui, frame)`; panels unified → `egui::Panel::
  top/bottom/left` + `.default_size(f)`; `Ui::scope_id` (not `id()`);
  `Id::unique` (not `Id::new`); `ui.allocate_rect(rect, Sense::
  click_and_drag())` replaces interact+`Response::ignore()`; `ctx
  .fonts_mut(|f: &mut FontsView| …)`; `ctx.open_url(OpenUrl::new_tab(
  url))`; `Visuals::noninteractive()` is a method; `Storage::
  set_string(k, String)`; epaint: `text::Fonts::new(TextOptions,
  FontDefinitions)` (NO `FontAtlas`), `Fonts::with_pixels_per_point`,
  no root `rect()` fn, `Shape::line_segment([Pos2; 2], stroke)`,
  `Align2` via `epaint::emath::Align2`, `Vec2::min_elem`, `Pos2::
  distance`, `Shape::visual_bounding_rect`, dashes only via
  `Shape::dashed_line_many(points, stroke, dash, gap, &mut out)`.
- Shell shape (`main.rs`): `Viewer` holds workspace/display/log/
  open_error/view_key/scene/scene_error/scheme/selection(paint::
  Selection, id-based)/hover/viewport(hit::Viewport)/needs_fit/
  restore_scale/pending_view/last_rect; canvas = allocate_rect →
  pan_by/zoom_at/pick; `App::save` persists theme+zoom+view keys
  `oxidizr.viewer/*`.
- Guard: 6 lib files (`lib`+`scene`+`hit`+`paint`+`commands`+`reload`),
  forbidden tokens `use eframe`/`eframe::`/`use egui`/`egui::` —
  epaint OK. Scene/hit/paint APIs: see module docs; `paint::
  PaintOptions { screen: egui::Rect, selection: Option<&Selection> }`.
- Clippy house traps: doc-list continuations exactly `//!   `;
  `assert!(x.is_empty())` BANNED → `assert_eq!(x, [] as [T; 0])`;
  `too_many_lines` allow must sit on the FN; `f32::from(f32_const)` =
  useless_conversion; nursery `format_push_string` → `write!`;
  `assigning_clones` → pass owned Strings in; per-fn
  `#[allow(clippy::cast_precision_loss)] // exact small indices`.
- Core view structs have NO `Default`; keep a resolved `View` binding
  alive while using its content (E0597). Tests: `#![allow(clippy::
  unwrap_used)]`. Float asserts only exactly-representable values.
- bless diff-0 check: `cargo xtask e2e bless 2>&1 | grep -cE ":
  unchanged$"` = 307 + clean `git status` under `testdata/`.
