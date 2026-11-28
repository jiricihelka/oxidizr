# oxidizr-viewer

Native workspace viewer + layout editor: open a Structurizr `.dsl` or
`.json`, look at every view, edit the layout — and on plain `.json`
sources also the dependency arrows — saving it back as JSON; a
DSL-backed workspace's model and the DSL text are never rewritten.

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
  filter, a view selector grouped by kind with search, and a details
  panel: selection facts, stored tags, properties, the description
  rendered as markdown and a URL link.
- Navigation: wheel zoom, drag pans, **right-drag** lassos, minimap,
  and ONE undo/redo history for the whole session — Ctrl+Z walks back
  through edits made in other views too.
- Layout editing: drag boxes (optional 10 px snap), reroute arrows by
  their handles: a selected arrow shows a dot on every segment
  (press-drag one to splice a waypoint and route it in one move),
  double-click inserts one, Alt+press deletes a vertex, arrow keys
  nudge (Shift = 10×) and **Ctrl+Shift+K** straightens the arrow. All
  edits live in a per-view overlay; views without stored coordinates
  open laid out by the bundled engine, and the **Layout** dialog runs
  it on demand (+ graphviz: `dot-backend` feature, `$OXIDZR_DOT_BIN`).
- Relationship editing on plain `.json` sources: hovering a box shows
  four port dots — drag one onto another box to CREATE a relationship
  (the new arrow lands selected); drag an endpoint square of a
  selected arrow to RECONNECT that end; **Delete** removes it. Editing
  a selected arrow's Description, Technology or this-view label
  happens in the details panel — Enter or clicking away commits one
  undoable step. The `model edited` chip marks unsaved model edits;
  Ctrl+S writes them into the opened `.json`.
- **Save** (Ctrl+S) writes edits back as JSON (in place, or a
  `<stem>.json` sidecar for DSL sources) through the same
  byte-contract writer as the CLI; **Export** writes the current
  view as `<stem>.svg` + `<stem>.png`, exactly as painted.
- Help: **F1** or `?` opens the keys & gestures overlay (the same
  table the empty-canvas hint reads; Esc closes it before clearing the
  selection), and hovering a route-edit dot, endpoint square or port
  dot names its gesture in the tooltip. **F2** (or the Documentation
  button in the details panel) opens the documentation window: the
  workspace's `!docs` sections plus those attached to the selection —
  markdown renders, AsciiDoc and format-less sections show verbatim.
- Live reload keeps layout, view and camera; viewport memory
  restores each view's pan/zoom from a `<stem>.oxidizr.json` cache.
- Keyboard: Ctrl+Z/Ctrl+Y undo/redo, Ctrl+S save, Ctrl/⌘-click
  opens an element URL, Esc closes help, then deselects.

## Development

The library core never references eframe/egui — a guard test enforces it.

```sh
cargo test -p oxidizr-viewer                 # lib core + golden paint
cargo run -p oxidizr-viewer --features gui   # the shell
```
