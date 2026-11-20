# oxidizr-viewer

Native workspace viewer + layout editor (P3): open a Structurizr `.dsl` or
`.json`, look at every view, edit the layout, save it back as JSON — the
model and the DSL text are never rewritten.

## Run

```sh
cargo run -p oxidizr-viewer --features gui -- workspace.dsl
cargo run -p oxidizr-viewer --features gui -- workspace.json --theme dark

# the unified binary drives the SAME window (F10, §P3-M8):
cargo run -p oxidizr-cli --features gui -- view workspace.dsl --theme dark
```

The `gui` feature compiles the eframe shell (`shell.rs`); the standalone
binary is its argv shim and `oxidizr view` is the in-process entry of the
unified `oxidizr` binary — both call one `oxidizr_viewer::shell::run`, so
they cannot drift. Without the cli `gui` feature `oxidizr view` still
appears in `--help` but exits with the rebuild hint. The library core is
headless — `cargo test -p oxidizr-viewer` never opens a window or touches
a GPU.

`--theme light|dark|system` (default `system`: persisted toggle first, then
the OS preference). A file argument opens at startup; otherwise use the
toolbar path field + Enter / **Open**.

## What you get

- Every view kind renders with full styling (custom/image views are
  placeholder boxes), light/dark themes, the bundled font, key/legend and
  tooltips, a tag filter, and the left panel: a **view selector grouped by
  view type** (System landscape → context → Container → Component →
  Deployment → Dynamic → Filtered → Image → Custom, each under its own
  heading) plus a search that jumps to elements (select + center) or
  switches to matching views.
- Navigation: wheel zoom, `−`/`+`/`Fit`, **right-drag** (or middle-button
  / Space-drag) to pan, minimap (click/drag recenters). Per-view undo
  history (cap 200).
- Editing: drag a box to move (optional **Snap**, 10 px grid), drag empty
  canvas to lasso, click/Shift-click to select, click an edge to select,
  drag its handles to reroute, double-click inserts a handle,
  Alt+click deletes one. Views whose file carries NO element coordinates
  open laid out by the bundled engine automatically (the view's
  `autolayout` settings when present) — a display default, never marked
  dirty; it is written out when you save. The **Layout** dialog runs the
  bundled layered engine on demand (direction + spacings, optional route
  vertices) and offers align/distribute/centre/bring-back/remove-vertices.
  All edits live in a per-view overlay — the opened workspace stays
  immutable in-app.
- **Save** (Ctrl+S, enabled when dirty): writes the layout back as JSON —
  the opened `.json` in place, or a `<stem>.json` sidecar for DSL sources —
  through the same byte-contract writer as the CLI (atomic temp+rename).
  A view that opened with automatic layout saves the COMPLETE geometry
  the screen showed — engine placements plus your edits.
- **Export**: writes `<stem>.svg` + `<stem>.png` with the diagram key
  beside the save file — the current view exactly as painted, no selection.
- Live reload: edits to the file on disk (including `!include` files)
  reload the view and KEEP your layout (reconcile remaps surviving ids).
  A saved `.json` sidecar becomes the open/reload source until the `.dsl`
  is edited again — newest wins. A failed reparse keeps the last-good view.

Keyboard: Ctrl+Z / Ctrl+Y undo/redo · Ctrl+S save · Ctrl/⌘-click opens an
element URL · Esc deselects. Toolbar `?` lists every gesture.

## Layout engines

The bundled `oxidizr-layout` `Layered` engine is always available.
Optionally, build with `--features dot-backend` and set `$OXIDZR_DOT_BIN`
to a graphviz binary: the Layout dialog then grows an **Engine** combo
(graphviz `dot`, via `-Tplain`). The choice is session-only; without the
feature or the variable the dialog looks exactly as before and the binary
spawns nothing.

## Development

```sh
cargo test -p oxidizr-viewer                                  # lib core + golden paint
cargo run -p oxidizr-viewer --features gui                    # the shell
cargo test --release -p oxidizr-viewer --test perf_scene -- --ignored --nocapture  # perf probe
```

Headless library modules (`scene`, `paint`, `hit`, `layout`, `commands`,
`reload`, `overview`, `export`, …) never reference eframe/egui — a guard
test enforces it; the shell in `main.rs` (feature `gui`) is the only
windowing surface. See `agents/plan/build/PHASE_3.md` for the design
decisions and `agents/notes/perf/scene-paint-budgets.md` for frame-budget
figures.
