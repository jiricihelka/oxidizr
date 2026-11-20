# RESUME — session checkpoint (updated 2026-11-20, **P3-M1 in progress → M1-5 next**)

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
  - **M1-4b DONE 2026-11-20**: headless `key` module — `legend(&Scene)`
    (deduped element/edge/boundary key rows, first-appearance order),
    `tags(&Workspace)` stored-tag inventory w/ counts,
    `session_mask(ws, view_key, mode, tags)` → chips evaluated with
    `filter::eval` on the view's RENDER BASE content (V5/V7:
    session-only, never persisted, intersects the view's own filter);
    5 unit tests incl. the Include trap (a relationship needs ITS OWN
    tags to hit — endpoints surviving is not enough). Shell: key panel
    (right, painter swatches), tag chips under the view list
    (exclude/include + clear → `rebuild_scene` with mask), diagnostics
    WINDOW toggled by the status-bar count (severity-colored via new
    `oxidizr-diag` dep). Guard now asserts 7 lib files.
  - **M1-5 NEXT**: SVG icons — `usvg` 0.50 + `tiny-skia` 0.12 rasterize
    to a texture atlas keyed (icon, size, scheme); placeholder = the
    dashed box `paint::icon_placeholder` already paints; missing icon
    file → warning diagnostic (reference logs it). Then the M1-close
    commit (doc rows + smoke checklist in the §P3-M1 close row).
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
  top/bottom/left/right` + `.default_size(f)`; `Ui::scope_id` (not
  `id()`); `Id::unique` (not `Id::new`); `ui.allocate_rect(rect,
  Sense::click_and_drag())`; `ctx.fonts_mut(|f: &mut FontsView| …)`;
  `ctx.open_url(OpenUrl::new_tab(url))`; `Visuals::noninteractive()`
  method; `Storage::set_string(k, String)`; `Window::new(..)
  .open(&mut bool)`; `Painter::rect_stroke(rect, r, stroke,
  StrokeKind)`; epaint: `text::Fonts::new(TextOptions,
  FontDefinitions)` (NO `FontAtlas`), no root `rect()` fn,
  `Shape::line_segment([Pos2; 2], stroke)`, `Align2` via
  `epaint::emath::Align2`, `Vec2::min_elem`, `Pos2::distance`,
  dashes only via `Shape::dashed_line_many`. `f32: From<i32>` does NOT
  exist (accumulate or cast-allow).
- Shell shape (`main.rs`): `Viewer` fields += `tag_inventory`/
  `tag_mode(FilterMode)`/`tag_sel`(SESSION)/`legend(key::Key)`/
  `show_diagnostics`; mask built in `rebuild_scene` only when
  `tag_sel` non-empty; `adopt` recomputes inventory + clears chips.
  Storage keys `oxidizr.viewer/{theme,zoom,view}` (chips NEVER
  persist, V7).
- `key::session_mask` follows the `scene::build` view/base resolution
  exactly (Filtered → base content); hold a function-lifetime `View`
  binding for the content borrow (E0597 trap).
- Guard: 7 lib files (`lib`+`scene`+`hit`+`key`+`paint`+`commands`+
  `reload`), forbidden tokens `use eframe`/`eframe::`/`use egui`/
  `egui::` — epaint OK.
- Clippy house traps: doc-list continuations exactly `//!   `;
  `assert!(x.is_empty())` BANNED → `assert_eq!(x, [] as [T; 0])`;
  `too_many_lines` allow must sit on the FN; `f32::from(f32_const)` =
  useless_conversion; nursery `format_push_string` → `write!`;
  `assigning_clones` → pass owned Strings in; per-fn
  `#[allow(clippy::cast_precision_loss)] // exact small indices`.
- Core view structs have NO `Default` (Model/ViewContent DO);
  `SoftwareSystem` has NO `aliases` field. Tests: `#![allow(clippy::
  unwrap_used)]`; integration fixture dirs must be PER TEST (parallel
  race). Float asserts only exactly-representable values.
- bless diff-0 check: `cargo xtask e2e bless 2>&1 | grep -cE ":
  unchanged$"` = 307 + clean `git status` under `testdata/`.
