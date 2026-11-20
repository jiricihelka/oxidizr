# RESUME — session checkpoint (updated 2026-11-20, **P3 + M6–M10 complete, tagged v0.3.0 → P4 next**)

## State

- **P1 COMPLETE** (tag v0.1.0), **P2 COMPLETE** (tag v0.2.0), **P2R
  COMPLETE** `410fee6`; **P3 COMPLETE 2026-11-20 — tagged `v0.3.0`
  (local)**:
  M0 `346ca6e`; M1 (`969a31b`→`496912c`); M2 (`c626048`→`468ec03`, `P3`
  bench baseline); M3 (`4fcb235`→`d39ebdc`); M4 (`81b0a3d`→`dc5d840`);
  M5: M5-0 `5f1dd32`, M5-1 `0842dbd`, M5-2 `2325947`, M5-3 `ff2b8b5`,
  close + tag. **Post-tag M6** (display auto-layout, §P3-M6): code
  `a1e7dfc`, docs `0083fe6`. **Post-tag M7** (§P3-M7): sidebar view list
  grouped per view kind, `e60dcba`. **Post-tag M8** (§P3-M8, F10):
  `oxidizr view` in-binary GUI subcommand — shell→lib `shell.rs`
  `55a0219`, cli entry `6bd7073`, gates/docs (this commit). **Post-tag
  M9** (§P3-M9): Windows CI reload fix — notify `Modify(Any)` class
  accepted on windows-cfg only + platform-mapping test. **Post-tag
  M10** (§P3-M10): display-layout separation floor (`DISPLAY_FALLBACK`
  200/120, graphviz lower-bound) + right-button pan (this commit).
  **Nothing pushed; tags v0.1.0/v0.2.0/v0.3.0.**
- **Ritual:** quartet every commit; shell commits also `cargo xtask ci
  gui`; lib commits also full tier (`OXIDZR_E2E_FULL=1 cargo test
  -p oxidizr-e2e` → 8 ok) + `cargo xtask e2e bless` → 307×unchanged +
  clean `testdata/`; commit every green sub-piece; **never push**.

## Next: Phase 4 (`build/PHASE_4.md` — authoritative, read it first)

- Scope: **F3 `diff`, F6 `query`, F7 `init`, F5 `lsp`, F4 `oracle` user
  command, inspect-parity**, diagram importers moved from P2
  (C4-PlantUML, Mermaid, image→CustomView), WebSequenceDiagrams
  exporter, release engineering (cargo-dist 3-OS). New crates per
  P4-D1: `oxidizr-import`/`-lsp`/`-inspect`; the rest are `oxidizr-cli`
  subcommands. diff exit-code adds `1 = differences found`.
- **Nothing may alter `oxidizr-json` bytes** (P4 opening clause) —
  bless 307×unchanged stays the tripwire.
- P4-M0 = decomposition table into PHASE_4.md (same ritual as P3-M0:
  docs commit, then per-item rows with DONE stamps at close).

## M5 hardening surface (xtask + probe — read before touching oracle verify)

- `cmd_oracle_verify` legs: json → **sidecar** → diagram → theme;
  summary `4 json + 4 sidecar + 90 diagram + 6 theme`. Sidecar leg
  (`verify_sidecar_legs` + `sidecar_edited_leg` + `collect_xy_maps`):
  open via `oxidizr_api`, save via `commands::write_workspace`;
  IDENTITY = reference re-export of our sidecar == re-export of the
  committed golden (both through `normalize_reference_output` —
  comparing RE-EXPORTS isolates our writer from reference quirks);
  EDITED = `layout::apply` → `apply_overlay` → save → reference load:
  `model` Value unchanged, id→x/y map survives verbatim AND differs
  from stored. xtask deps: oxidizr-api/core/viewer + serde_json
  (dep-table changes ⇒ stage `Cargo.lock` with the commit).
- Perf probe `crates/oxidizr-viewer/tests/perf_scene.rs` (ignored):
  `cargo test --release -p oxidizr-viewer --test perf_scene -- --ignored
  --nocapture`. Figures in `notes/perf/scene-paint-budgets.md` — 209
  items + 247 edges ≈ 1.0 ms/frame (~16× headroom); INFORMATIONAL
  (P3-D0), never a golden.

## M4-6 dot-backend surface (dot.rs + layout.rs — read before touching)

- `oxidizr-layout` feature `dot-backend` (OFF by default, adds only
  `std::process`): `Dot::from_env()` = `$OXIDZR_DOT_BIN` (empty ⇒
  None, oracle posture); `dot_source()` = input order, positional
  names `n0..` (ids NEVER enter the source), graph attrs
  rankdir/nodesep/ranksep/esep (÷ 72 px/in), splines from
  `generate_vertices`, tailport/headport for anchors.
- **`-Tplain` reality (graphviz 15.1.1, verified 2026-11-20 — NOT the
  man page):** node rows carry FULL extents (top-left = `(x-w/2,
  graph_h-y-h/2)`), edge points are TWO plain numbers (docs' joined
  `x,y` also parsed), the FIRST `graph` block starts at the origin
  (gap only BETWEEN components, 50 px), y is bottom-up, inches.
- `parse_plain` maps back in input order; parallel edges FIFO per
  (src,dst) pair. Category B test `real_dot_binary…` gated on the env
  var; determinism = fixed graphviz build.
- Viewer: feature passthrough `dot-backend`; `layout::Engine`
  (Layered always; Dot only feature + env) — `available()` powers the
  dialog combo (appears iff > 1 engine), `apply_with()` is the one
  translation pipeline (`apply()` = Layered, headless tests
  unchanged); Dot w/o env ⇒ `LayoutError::DotBackend` value. Engine
  choice SESSION-ONLY — `encode`/`decode` stay `"LR|100|50|50|1"`.

## M4-4/M4-5 shipped surface (overview.rs + export.rs)

- `overview::Overview::new(Scene::bounds, ERect) -> Option`: uniform
  fit + centred; `shapes(scene, view_world, selection)`; `map_to_world`
  inverse drives `Drag::Minimap` recenter. Quick-nav `nav_matches`
  feeds the left-panel search; field `text_focus` ORs toolbar + nav
  focus; guards Ctrl+Z/Y/S.
- `export::compose(scene, &mut FontsView) -> Composed{shapes,size}`:
  fitted `Viewport` (1:1 → 4096 cap, floor 0.25, 80 MP; degenerate ⇒
  400×240; selection NEVER painted); key band composed at ORIGIN then
  `Shape::translate(0, fig_h + KEY_TOP_PAD)` — never forget the
  translate. `shapes_to_svg` covers the closed Shape subset; text =
  per-glyph outlines via `ab_glyph` over the bundled Ubuntu-Light
  (workspace pin names `default_fonts` — READ
  `pitfalls/epaint-037-font-stack.md` before touching fonts/epaint).
  `svg_to_png` = usvg → resvg → `encode_png`; gate = parse OK + PNG
  magic (bytes NOT goldens, P3-D0). Shell: toolbar Export ⇒
  `<stem>.svg/.png` beside `commands::save_target` via
  `commands::write_atomic` (shared with save; `.{pid}.tmp`+rename
  never matches the watch filter). Notes: success `exported …`,
  fail/blocked `export …` (dark-red prefix set: `serialize:`,
  `write `, `export `).

## Reload stack (M4-1..M4-3 — read before touching reload)

- `ParseResult.includes` = FS `!include` files in parse order;
  `Opened.includes` carries it (empty for JSON/stdin).
- `reload.rs::ReloadDriver` (headless, notify, 250 ms settle-debounce,
  parent-dir NonRecursive + exact-path filter, SipHash baselines are
  the authority — `interesting()` is a PRE-filter only; on Windows the
  backend never emits `Modify(Data)`, plain writes are `Modify(Any)`, so
  that class is accepted under `#[cfg(target_os = "windows")]` (§P3-M9,
  pitfalls/notify-windows-modify-any.md). `Wake = Arc<dyn Fn() + Send +
  Sync>` (eframe 0.37 has NO RepaintSignal; shell passes `move ||
  ctx.request_repaint()`, `repaint_ctx` stored first frame). R-2 guard
  scans doc literals.
- Shell: poll hook + re-arm guard in `App::ui`; `adopt` drops the
  driver; save Ok-arm `restart_watch()` re-baselines (self-save
  hash-matches ⇒ no self-reload); watch failure ⇒ `reload_disabled`.
  `reload_from_disk` opens `reload_source()` = NEWEST of
  display/sidecar/companion `.dsl` (ties ⇒ sidecar); folds overlays →
  `reconcile` → re-keys overlays via `Report.remapped_elements`/
  `remapped_relationships` (PER KIND) → `adopt` → restore overlays +
  `rebuild_scene()`; failed reparse keeps last-good + note.
  `companion_dsl` keeps an opened sidecar's `.dsl` watched.

## M3 editing core (commands.rs + shell — read before touching edits)

- `LayoutOverlay { view_key, elements (top-lefts), relationships
  (interiors), dimensions: Option<Dimensions> (Copy!) }`; readers
  `position`/`rect`/`interior` overlay-first; `as_overrides()` →
  `scene::build` 5th param. `Command` set symmetric `apply(forward)`;
  factories ALL `Option<Command>`; `History` cap 200; per-view
  overlay/history/dirty, cleared at adopt only.
- SAVE (M3-5): `apply_overlay` writes elementViews x/y +
  relationshipViews vertices + dimensions (`to_px` round+clamp i32),
  walks the 7 MODEL-view vecs by `base.key`; filtered/image ⇒ save
  BLOCKED (`current_view_filtered`). `save_target`: `.json` in place,
  `.dsl` → sidecar, `-` → None. `write_workspace` =
  `oxidizr_json::to_vec` + `write_atomic`. BYTE STABILITY test:
  `to_vec(from_slice(bytes)) == bytes` (writer sorts by id — struct
  equality is NOT the contract). Ctrl+S focus-guarded; Save iff dirty.

## Clippy/lint catalog (cumulative — `ci gui` alone catches shell docs)

- egui 0.37: `PointerButton::Primary`; `i.pointer.any_down()`;
  `Ui::toggle_value`; `rect_stroke(rect, rounding, stroke,
  StrokeKind)`; `Id::new` DEPRECATED (`response.has_focus()`);
  `double_clicked()`; `ui.add_enabled(bool, egui::Button::new(..))`;
  `Context::request_repaint()` IS the cross-thread wake; NO
  `strong_noninteractive_color`; `Shape::translate(&mut self, Vec2)`;
  `CornerRadius` = `nw/ne/sw/se: u8`; `Shape::line` = OPEN Path.
- `similar_names` DENY (`tiles`/`tiled` tripped it); `type_complexity`
  ⇒ alias or fn-`#[allow]` + comment; `redundant_clone` early-return;
  `redundant_closure` (is_some_and(T::method));
  `items_after_statements` — fn-body `use` goes FIRST;
  `assigning_clones`/`clone_on_copy` — `Dimensions` IS Copy;
  `doc_lazy_continuation` for doc lines starting `+`; E0282 on bare
  `Vec::new()` in assert_eq ⇒ bind typed local; `cargo fmt` reflows —
  match post-fmt text; `manual_div_ceil` ⇒ `.div_ceil()`; usize→f64/
  u32 casts ⇒ `f64::from(u32::try_from(x).unwrap())`;
  `too_many_lines` — split a helper (M5-1) before allowing.
- `format_push_string` ⇒ `let _ = write!`/`writeln!` (a format ending
  in `\n` MUST be `writeln!`); `trivially_copy_pass_by_ref` — `Stroke`
  (8 B) by VALUE; `f32: TryFrom<f64>` is NOT std — own saturating
  helper; `doc_markdown` backticks (`ab_glyph`); Result fns need
  `# Errors`; `collapsible_if` via let-chains; `too_many_lines` allow
  needs a rationale comment; unused `mut` when a cfg block vanishes —
  restructure instead of `#[allow]`.
- R-2 guard: lib core NEVER references eframe/egui (literals in DOC
  COMMENTS fail); the skip-set is the two `gui`-gated shell files
  {`main.rs`, `shell.rs`} (§P3-M8); count assert = lib.rs + N modules —
  NOW 11 files (the shell module is NOT a core module).

## M6 display auto-layout (layout.rs + shell — read before touching)

- Rule (mirrors structurizr web): a view whose RENDERED content (filtered
  → base) has NO non-zero stored coord opens laid out by the bundled
  Layered engine — the loader materializes a missing `location` as the
  `0`/`0` pair, so all-zeros = "file has no layout". Display is
  session-only, NEVER dirty; views WITH stored coords render verbatim
  (`autolayout_once` is GONE — it dirtied on open and only fired with a
  stored `automaticLayout`). Params = view `automaticLayout` else the
  reference LR defaults, separations FLOORED at `DISPLAY_FALLBACK`
  (rank 200/node 120, graphviz lower-bound §P3-M10: cramped `autoLayout`
  100/50 rides up; direction/vertices stay the file's; dialog +
  `AutomaticLayout::default` untouched) — NEVER the dialog's session
  params; engine = Layered only, never `dot`.
- Seam: `view_needs_layout` / `display_params` / `auto_display` /
  `seed_overlay` (save-side); `Overrides::display(auto, user)` merges the
  overlay OVER the engine PER ID. Shell: `rebuild_scene` runs the pass
  UNDER the overlay; `save_workspace` seeds untouched ids so the JSON
  persists what was displayed. Guard test: `tests/auto_display.rs`
  (big-bank-plc Components: pile → spread → pin → seed).
- **M7**: the sidebar list is partitioned by kind — `overview::section`
  (rank+header per `View` variant, C4 granularity first) +
  `overview::grouped` (stable `(rank, order, key)` sort → header runs);
  the shell renders separator + uppercase headings, rows via `view_row`.

## M8 `oxidizr view` (F10 — shell now a lib module; read before touching)

- The eframe shell MOVED `main.rs` → `src/shell.rs`, `#[cfg(feature =
  "gui")] pub mod shell` (lib). Public surface: `shell::run(Option<&Path>,
  ThemeArg) -> eframe::Result` + `pub enum ThemeArg`. `main.rs` is a thin
  argv shim (clap `Args` → `shell::run`); the CLI's `oxidizr view`
  (`oxidizr-cli` feature `gui`) calls the SAME `shell::run` in-process —
  entries cannot drift. R-2 guard skip-set is now the two shell files.
- CLI: `oxidizr-cli` gained `gui = ["dep:oxidizr-viewer",
  "oxidizr-viewer/gui"]` + `Command::View { file, --theme }`. `run()`
  branches on `View` BEFORE the pipeline (`to_request` → `Option`, None
  ONLY for view) — the GUI is NOT an `oxidizr-api::Request`. Without the
  cli `gui` feature, `view` is stable `--help` surface but exits 1 with
  the rebuild hint (`anyhow::bail!`, cfg-gated). `ViewTheme` mirrors
  `ThemeArg`.
- Gates: `cargo xtask ci gui` now clippies `-p oxidizr-viewer -p
  oxidizr-cli --features oxidizr-viewer/gui,oxidizr-cli/gui` (both);
  `.github/workflows/ci.yml` mirrors the gui clippy + the 3-OS check.

## Viewer orientation (M1/M2 shipped)

- Scene: item rect top-left x/y (or override), size = style else
  350×150; edge polyline = [source centre, interiors…, destination
  centre]; `hit::pick` items last-wins, edges tolerance-first;
  boundaries not pickable. `layout.rs` `stored_for`/`config_for`/
  `apply`/`encode`/`decode` + the M6 display seam above; boundary pad
  `BOUNDARY_PADDING = 20` (deviation). Core↔engine direction names
  differ — `layout::direction()` maps.
- eframe-0.37: unified `egui::Panel`; `Storage::set_string(k, String)`;
  `Window::open(&mut bool)`; `ctx.load_texture`;
  `ComboBox::from_id_salt`; click after a MOVED drag never fires;
  bless diff-0 = `cargo xtask e2e bless | grep -cE ": unchanged$"` ==
  307 + clean `git status testdata/`.
- `cargo test -p oxidizr-viewer` = 95 lib + 1 auto_display + 3 golden
  (+2 ignored perf);
  `cargo test -p oxidizr-layout --features dot-backend` = 31.
  Viewer usage lives in `crates/oxidizr-viewer/README.md` (M5-3).
