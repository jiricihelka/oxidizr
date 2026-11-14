# PHASE_3.md — detailed implementation plan: native GUI (viewer + layout editing)

Scope per PLAN.md Phase 3, decisions taken 2026-11-14: **egui** toolkit (O1 closed),
**own pure-Rust layered layout engine behind a `LayoutEngine` trait with an optional
`dot-backend` cargo feature** (O2 closed), and the **full feature list**: open + watch
workspace files with live reload (layout preserved via the F1 remap), undo/redo for
layout edits, bundled font (no system-font dependency), PNG/SVG icons, light/dark
follow-OS, save writes standard JSON geometry — so edited layouts stay readable by
official tooling (the compat gate applies to every save).

Single-binary promise: `oxidizr-viewer` is one self-contained executable — no Java, no
browser, no mandatory external binaries. GUI export buttons and multi-window are
**deferred** (out of this phase).

## P3-D1 Crate topology

```
crates/
  oxidizr-layout   pure graph + layered (Sugiyama-style) layout engine; deps: none
                   (neutral graph types in/out; no core dep, no IO, no GUI)
  oxidizr-viewer   BINARY: eframe/egui app — scene, interaction, editing, watching;
                   deps: core, json, styles(core), layout, notify, usvg/tiny-skia,
                   dark-light; NEVER depended on by other crates
```

- All non-trivial logic lives in `oxidizr-viewer`'s **library core modules**
  (`scene`, `hit`, `commands`, `reload`) which import nothing from eframe — the eframe
  integration layer is a thin shell. Category A tests exercise scene building,
  hit-testing, undo/redo and remap logic headlessly; only pixel presentation is manual-
  QA. `cargo test --workspace` must build the viewer with zero GPU/windowing need at
  test time (eframe behind the bin target only).
- `oxidizr-cli` gains no GUI dependency; a future `oxidizr view` launcher is out of
  scope (💡 idea if demanded).
- **Reconcile engine relocation:** the id-remap matching logic from P1-D8 (currently in
  `cli`) moves to `oxidizr-core::reconcile` (pure model operations; core stays
  dependency-free) at P3-M0 so viewer and CLI share one implementation. Tests move
  with it. Noted in DEVIATIONS.md? No — internal structure, not behavior; recorded
  here only.

## P3-D2 egui stack specifics

- `eframe` with the **wgpu** renderer (better HiDPI/text path; record the chosen
  version pin in `DEPENDENCIES` rustdoc of the crate). Glow stays a fallback feature
  only if wgpu proves problematic on CI runners (no CI needs a GPU — see P3-D8).
- "No `unsafe` in oxidizr code" applies unchanged; GPU dependencies contain unsafe
  internally, which is outside the rule's scope — one clarifying sentence goes into
  AGENTS.md at P3 start (guideline wording touch-up, no behavior change).
- Event-driven repaint (`eframe` default) — no continuous redraw loop; idle window
  costs nothing (portability: laptop battery is a platform concern).
- Text: egui's **bundled fonts only** (Ubuntu-family embedded). Theme `fontUrl`
  handling: offline-first (D5) → without `--allow-net`, bundled font is substituted
  with one Info diagnostic per theme; JSON output never changes (fontUrl is stored
  verbatim). Verification item P3-V2 checks reference behavior for parity intent.
- Window geometry, zoom and theme override persist via **eframe's built-in storage**
  only — no custom settings file (AGENTS.md CLI/config policy: no config files).

## P3-D3 Scene architecture (build once, paint cheap)

```
Workspace + resolved styles (core::styles, P2-D2)
  → Scene { layers, items: [ElementItem{rect, icon?, text-runs, style}],
            edges: [EdgeItem{polyline, label, style}], scale-independent metrics }
  → paint: viewport-culled traversal issuing epaint shapes/textures
  → hit-test: hand-rolled AABB + point-to-segment (no geometry crate)
```

- Scene is **retained**; rebuilt only on model change, layout edit (targeted item
  update), or zoom-band transition (label LOD thresholds), never per frame.
- Viewport culling + one texture atlas for rasterized icons; text layout via
  `epaint::LayoutJob` cached inside items (wrap widths recompute on resize only).
- Filtered views compute their included set by evaluating the stored filter (core
  filter evaluator; if P1 stored filters unparsed, P3-M0 adds `core::filter::eval`
  reusing the DSL filter grammar — same code path `query` will use in P4).
- Dynamic views: numbered sequential edges + prev/next animation stepper (frames =
  `automaticLayout`-independent order/index); deployment views render nodes/instances
  (nested containers); custom/image views: placeholder box with the stored definition
  or image URL (full plantuml-in-view rendering deferred — note, not a layout case).

## P3-D4 Editing model: overlay + commands, immutable workspace untouched

- The editor holds a `LayoutOverlay` (id → geometry deltas) on top of the read-only
  `Workspace`. All edits are invertible commands (`MoveElement`, `SetWaypoints`,
  `InsertWaypoint`, `DeleteWaypoint`, `SetAnchor`, `ClearViewLayout`, grouped
  multi-select moves); `Vec` undo/redo stacks, cap 200, Ctrl+Z/Ctrl+Shift+Z/Ctrl+Y.
- Save = clone workspace → apply overlay (elementViews x/y/width/height,
  relationshipViews waypoints/order/orientation) → **`oxidizr-json` writer** → byte-
  compatible standard JSON (WRITE-CATALOG rules apply unchanged; the save path adds no
  serializer). Target file: the layout JSON ("layout file" workflow — open DSL →
  in-memory workspace → save `<name>.json` sidecar layout, exactly the file
  `reconcile --dsl X --layout Y` consumes). Manual save only (Ctrl+S); autosave
  deferred.
- Interaction: left-drag elements (grid snap 10 px, toggle), click-select +
  box-select, wheel zoom-at-cursor, middle/space-drag pan, waypoint insert/drag/
  delete on edges, "Apply auto-layout" per view and for all views, "Clear layout".

## P3-D5 Layout engine (`oxidizr-layout`)

- Public surface: `LayoutGraph { nodes(…size), edges(…from/to anchors), rank_direction,
  rank_separation, node_separation } → LayoutResult { node positions, edge polylines
  incl. waypoints }`; `trait LayoutEngine { fn layout(&LayoutGraph) -> … }` with two
  impls: `Layered` (own) and `Dot` (feature `dot-backend`, subprocess `dot -Tplain2`
  /`-Tplain-ext`-class output, `$OXIDZR_DOT_BIN` → PATH discovery, absent → diagnostic
  suggesting the plain build; never on by default — single-binary promise).
- `Layered` = compact-priority Sugiyama: cycle removal by DFS reversal, layering
  (longest-path + network-simplex-lite iteration cap), ordering (barycenter w/
  deterministic tie-breaks), x-coordinates (priority method, separation constraints),
  orthogonal edge routing along rank gaps; rankDirection maps `automaticLayout`
  LeftRight/RightLeft/TopDown/BottomUp; separations from view `automaticLayout`.
- **Determinism is a hard contract:** no randomness, stable sorts, fixed iteration
  counts → property test "same input ⇒ byte-identical layout" + criterion benches on
  large synthetic views (1k nodes) — the mandated `reconcile`-class bench targets
  extend to `layout`.
- Quality bar is "usable C4 diagrams" (shallow layered graphs), not Graphviz parity;
  the C4 rank semantics (which direction boundaries nest, group handling as clustered
  subgraphs via boundary containers) verified against reference renders — P3-V1.

## P3-D6 Live reload preserving layout

- `notify` watches the opened file **plus the whole `!include` closure** (the P1
  loader records the resolved file set); 250 ms debounce per path-set.
- On change: full reparse/rebuild → `core::reconcile` maps old ids → new ids
  (same rules as the CLI command: `structurizr.dsl.identifier` exact, canonical-name
  fallback, ambiguous = reported, never guessed) → surviving items keep overlay
  geometry, added items get auto-layout for their view, removed items drop; UI badge
  lists added/removed/ambiguous, Info diagnostics surface parse regressions without
  closing the window (last-good workspace stays visible).
- Saving from the viewer must not self-trigger a reload storm (write via temp+rename,
  ignore our own events by content-hash compare).

## P3-D7 Icons, theme, light/dark

- Icons: PNG/other rasters via `image`; **SVG via `usvg` + `tiny-skia`** rasterized
  once per (icon, size, scheme) into the atlas. SVGs containing text glyphs render
  without text (usvg text needs a font stack we deliberately don't carry) — recorded
  limitation + warning diagnostic (P3-V3 checks how often theme icons contain text —
  expected: never).
- Full styling coverage reuses P2's `core::styles` (shapes, borders, stroke, opacity,
  iconPosition, metadata rows, terminology, group labels); default theme bundled.
- Light/dark: `dark-light` crate reads the OS preference at startup (its
  platform-specific code is dependency-internal; our call site gets a justification
  comment per the portability rule), flag `--theme light|dark|system` (default
  system→fallback light), toggleable at runtime; both schemes' atlases cached lazily.

## P3-D8 Testing & QA

- Category A: unit + snapshot (scene graph `expect-test` per fixture view), property
  (deterministic layout, undo/redo group inverse, reload-remap keeps geometry),
  golden: saved JSON after scripted edit sequences byte-compared against stored
  goldens (the geometry contract is JSON — full byte-parity bar returns here).
- Category B: viewer-saved files load in the reference tool (`xtask oracle verify`
  extended to "edited workspace" fixtures).
- Manual QA checklist per milestone (windowing, drag feel, HiDPI, 3-OS smoke) — CI
  matrix keeps building the viewer (`cargo check`/`clippy` all targets) but runs no
  GUI; no xvfb pixel tests (flaky, low value vs scene-graph snapshots).

## P3-D9 Milestones (ordered, each ends green)

- **M0** prep: move reconcile engine into core; `oxidizr-layout`/`oxidizr-viewer`
  scaffold; CI builds viewer on 3 OSes headless; core `filter::eval` if missing.
- **M1** read-only viewer: open DSL/JSON, scene build, pan/zoom/select, all view
  kinds render with full styling, light/dark, bundled font, icons (PNG+SVG).
- **M2** `Layered` engine: algorithm complete, determinism property, benches +
  committed baseline (P3 joins the milestone baseline list), auto-layout buttons.
- **M3** editing: overlay + commands, undo/redo, snap, waypoint editing, save →
  byte-compatible JSON; oracle round-trip check green.
- **M4** live reload + remap UX; DSL→sidecar-layout workflow; `dot-backend` feature
  (optional) + docs.
- **M5** hardening: perf pass against big-bank-plc/spring-petclinic/aws corpus
  (scene rebuild + paint budget: 60 fps at 200 visible elements — measured with
  `eframe` stats overlay, informational), docs, tag v0.3.0.

## P3-D10 Verification list (empirical, before dependent code)

- P3-V1 auto-layout parity intent: what the reference web tool/dot produces for
  representative C4 views (rank direction per view type, boundary nesting) — drives
  `Layered` tuning; result is our {x,y}, contract-safe by definition.
- P3-V2 reference font/theme loading (offline story) → confirm bundled-substitution
  policy in P3-D2.
- P3-V3 theme icon corpus scan: SVG-with-text frequency; confirm atlas approach.
- P3-V4 `automaticLayout` fields honored by reference consumers when geometry exists
  (do we clear or keep the block on manual edit? match Lite behavior).
- P3-V5 filtered-view evaluation semantics corner cases (relationship filters,
  `directRelationshipsOnly`) vs `core::filter::eval`.
- P3-V6 `notify` event coalescing behavior on each OS (debounce tuning), Windows
  rename semantics on save.

## Risks

- R-1 layout quality disappoints on big nested graphs (mitigation: editing is the escape hatch by design; `dot-backend` optional; shallow-graph focus).
- R-2 GUI logic leaking into the eframe shell, killing headless tests (mitigation:
  P3-D1 module boundary enforced by review + `oxidizr-viewer` lib/bin split lint).
- R-3 reload-remap mis-assigning geometry (mitigation: shared core::reconcile,
  ambiguous = conservative drop + report, property tests).
- R-4 wgpu/eframe API churn (mitigation: version pin, thin shell, upgrade as a
  planned chore).
- R-5 SVG edge cases (mitigation: warn-and-placeholder fallback path from day one).
