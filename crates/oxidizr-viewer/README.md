# oxidizr-viewer

Native workspace viewer + layout editor: open a Structurizr `.dsl` or
`.json`, look at every view, edit the layout, save it back as JSON —
the model and the DSL text are never rewritten.

```sh
cargo run -p oxidizr-viewer --features gui -- workspace.dsl
cargo run -p oxidizr-cli --features gui -- view workspace.dsl --theme dark
```

The `gui` feature compiles the eframe shell; the standalone binary and
`oxidizr view` share one entry point. The core is headless — tests
never window or touch a GPU. `--theme light|dark|system`, default `system`.

## What you get

- Every view kind renders fully styled: arrows land on box sides,
  labels paint above every box, light/dark themes, tooltips, a tag
  filter, a view selector grouped by kind with search, selection
  facts in the right panel.
- Navigation: wheel zoom, drag pans, **right-drag** lassos, minimap,
  per-view undo history.
- Editing: drag boxes (optional 10 px snap), reroute edges by their
  handles, double-click inserts one, **Delete** drops them. All edits
  live in a per-view overlay; views without stored coordinates open
  laid out by the bundled engine, and the **Layout** dialog runs it
  on demand (+ graphviz: `dot-backend` feature, `$OXIDZR_DOT_BIN`).
- **Save** (Ctrl+S) writes the layout back as JSON (in place, or a
  `<stem>.json` sidecar for DSL sources) through the same
  byte-contract writer as the CLI; **Export** writes the current
  view as `<stem>.svg` + `<stem>.png`, exactly as painted.
- Live reload keeps layout, view and camera; viewport memory
  restores each view's pan/zoom from a `<stem>.oxidizr.json` cache.
- Keyboard: Ctrl+Z/Ctrl+Y undo/redo, Ctrl+S save, Ctrl/⌘-click
  opens an element URL, Esc deselects; toolbar `?` shows gestures.

## Development

The library core never references eframe/egui — a guard test enforces it.

```sh
cargo test -p oxidizr-viewer                 # lib core + golden paint
cargo run -p oxidizr-viewer --features gui   # the shell
```
