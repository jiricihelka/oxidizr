# PHASE_3.md — detailed implementation plan: native GUI (viewer + layout editing)

Scope per PLAN.md Phase 3, decisions taken 2026-11-14: **egui** toolkit (O1 closed),
**own pure-Rust layered layout engine behind a `LayoutEngine` trait with an optional
`dot-backend` cargo feature** (O2 closed), and the **full feature list**: open + watch
workspace files with live reload (layout preserved via the F1 remap), undo/redo for
layout edits, bundled font (no system-font dependency), PNG/SVG icons, light/dark
follow-OS, save writes standard JSON geometry — so edited layouts stay readable by
official tooling (the compat gate applies to every save).

Single-binary promise: `oxidizr-viewer` is one self-contained executable — no Java, no
browser, no mandatory external binaries. GUI export is **in** (PNG/SVG with key/legend,
signed off 2026-11-18, delivered at M4); multi-window stays deferred.

## P3-D0 Parity contract: feature parity, NOT visual parity (ratified 2026-11-18)

**We do not chase visual parity with the Structurizr web UI** — its GUI is unintuitive
and wonky. We clone **what it can do** (feature parity), never **how it looks**:

- Native egui look-and-feel wins over any web mimicry — chrome, panels, buttons,
  modals, keyboard scheme are ours to design for usability (the reference stays the
  feature checklist, not the design mockup).
- **Exception — the diagram canvas is data, not styling.** Elements render from the
  workspace JSON via P2's `core::styles` (theme colors/shapes/icons, terminology,
  metadata, legends). That is the JSON compat contract applied, not visual parity.
- No pixel/CSS/font-match tests exist for the viewer. QA = the feature matrix below
  (each ✅ row demonstrably works on real workspaces) + the manual QA checklist.
- Where the reference has *no* feature and we need one for a good native app
  (view selector ergonomics, file dialogs), it goes through
  `ADDITIONAL_FEATURES.md` as usual.

**Editor scope boundary (signed off 2026-11-18):** every editing feature ships only if
it changes **values in the layout JSON** (view geometry, view settings) and **never
the `.dsl` model**. Anything needing model edits is out of viewer scope, by rule.

**Sign-offs Q1–Q4 (2026-11-18, user):** Q1 theme data drives the canvas (data-driven
compat, not mimicry). Q2 PNG/SVG export **in P3** (diagram + key, M4). Q3 all viewing
and navigation features are **in P3** — the boundary is the editor rule above, not
effort: legend, tooltips, tag filter, align/distribute/centre, canvas size, **minimap
and quick-navigation included** (P4 retains only polish). Q4 image views and custom
views both render as **placeholder boxes** (no image loading, no embedded
plantuml/mermaid/d2 renderers — single-binary promise).

## P3-F Feature parity matrix (verified against pinned monorepo, 2026-11-18)

Sources: `webapp/WEB-INF/fragments/diagrams/*.jspf`, `webapp/WEB-INF/jsp/{diagrams,tree}.jsp`,
`static/js/structurizr-diagram.js`, `structurizr-core/.../view/ViewSet.java`.

| #  | Reference feature                       | Source (line)                       | P3 stance                        |
| -- | --------------------------------------- | ----------------------------------- | -------------------------------- |
| 1  | All 9 view kinds render (landscape…image) | `ViewSet.java:735–860`             | ✅ all; custom+image = placeholders |
| 2  | View navigation tree / switching        | `tree.jsp`, `navigation.jspf`       | ✅ view selector (P3-D4)          |
| 3  | Pan + wheel zoom                        | `structurizr-diagram.js` ZoomScale  | ✅ (P3-D4)                        |
| 4  | Diagram key/legend overlay `[i]`        | `key.jspf`                          | ✅ IN (Q3, M1)                    |
| 5  | Tooltips on/off `[t]`                   | `tooltip.jspf`                      | ✅ IN (Q3, M1)                    |
| 6  | Interactive tag filter (client-side)    | `filter.jspf`                       | ✅ IN (Q3, M1; P3-V7 persistence) |
| 7  | Dynamic-view animation (2 s autoplay)   | `structurizr-diagram.js:80–111`     | ✅ stepper (P3-D3) + play button  |
| 8  | Edit mode (layout lock/unlock)          | `controls.jspf:61`                  | ✅ (P3-D4)                        |
| 9  | Drag elements, lasso select, snap       | `lasso.jspf`, P3-D4                 | ✅ (P3-D4)                        |
| 10 | Align L/C/R/T/M/B + distribute H/V + centre | `controls.jspf:85–111`          | ✅ IN (Q3, M3 commands)           |
| 11 | Canvas size ±/auto (view dimensions)    | `controls.jspf:74–76`               | ✅ IN (Q3, M3; writes JSON dims)  |
| 12 | Auto-layout dialog (direction, generate vertices, rank/node separation) | `auto-layout.jspf` | ✅ dialog → P3-D5 params |
| 13 | "Bring elements back" / "remove all vertices" | `auto-layout.jspf:53–54`     | ✅ commands (M3)                  |
| 14 | Undo layout changes `[u]`               | `controls.jspf:116`                 | ✅ (P3-D4)                        |
| 15 | Save workspace (layout survives)        | `controls.jspf:127`                 | ✅ (P3-D4; sidecar/in-place)      |
| 16 | PNG/SVG export (diagram + key)          | `export.jspf`, `controls.jspf:45`   | ✅ IN (Q2, M4; P3-V8 reference look) |
| 17 | Diagram navigator (minimap modal)       | `navigator.jspf`                    | ✅ IN (Q3, M4)                    |
| 18 | Quick navigation (search)               | `quick-navigation.jspf`             | ✅ IN (Q3, M4)                    |
| 19 | Perspectives                            | `perspectives.jspf`                 | ❌ N/A — service-side feature     |
| 20 | Health-check overlay                    | `structurizr-healthcheck.js`        | ❌ N/A — P4 `inspect` covers      |
| 21 | Explore (adjacency walk)                | `controls.jspf:21`                  | ❌ out; 💡 P4+ candidate          |
| 22 | Embed / share / home / summary          | web chrome                          | ❌ N/A — native app has files     |
| 23 | Multi-user lock, git sync, DSL editing  | workspace services                  | ❌ N/A — CLI covers (`inspect`, P4) |

Sign-offs closed 2026-11-18 — see P3-D0 (Q1 theme data ✅, Q2 export ✅ M4, Q3 all
viewing/navigation ✅ incl. minimap + quick-nav, Q4 placeholders). The matrix is final;
adding/removing a ✅ row now requires a user decision, not an implementation choice.

## P3-D1 Crate topology

```
crates/
  oxidizr-layout   pure graph + layered (Sugiyama-style) layout engine; deps: none
                   (neutral graph types in/out; no core dep, no IO, no GUI)
  oxidizr-viewer   BINARY: eframe/egui app — scene, interaction, editing, watching;
                   deps: core, api, json, styles(core), layout, notify, usvg/tiny-skia,
                   dark-light; NEVER depended on by other crates
```

- All non-trivial logic lives in `oxidizr-viewer`'s **library core modules**
  (`scene`, `hit`, `commands`, `reload`) which import nothing from eframe — the eframe
  integration layer is a thin shell. Category A tests exercise scene building,
  hit-testing, undo/redo and remap logic headlessly; only pixel presentation is manual-
  QA. `cargo test --workspace` must build the viewer with zero GPU/windowing need at
  test time (eframe behind the bin target only).
- `oxidizr-cli` gains no GUI dependency; a future `oxidizr view` launcher is out of
  scope (💡 idea if demanded). **AMENDED 2026-11-20 (§P3-M8, user-signed): the idea was
  demanded — `oxidizr view` lands as an OPTIONAL `gui` feature of `oxidizr-cli`
  (default cli build stays zero-GUI; the eframe shell moves to a `gui`-gated lib
  module so the in-process entry and the standalone binary share one `run`).**
- **Reconcile engine — NO relocation (ratified 2026-11-19, M0 planning).** Premise
  correction: the P1-D8 id-remap engine has lived in `oxidizr-api/src/reconcile.rs`
  since P1-M7 — never in `cli` — and is already `pub`, shared by the CLI via
  `api::pipeline`. A move to core is off the table: `oxidizr-core` carries the LOCKED
  zero-dependency promise (P1-D1) while the engine consumes `oxidizr_diag::codes` and
  api's `Issue`. The viewer depends on `oxidizr-api` anyway (it opens files through
  the pipeline — the deps list above gained `api`), so "viewer and CLI share one
  implementation" already holds; M0-1 only records this decision. Internal structure,
  not behavior — DEVIATIONS.md untouched.

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
  only — no custom settings file (agents/plan/WORKFLOW.md §CLI/config policy: no config files).

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
- Dynamic views: numbered sequential edges + prev/next animation stepper **and an
  autoplay toggle** (frames = `automaticLayout`-independent order/index; reference
  cadence is 2 s/step, ours adjustable); deployment views render nodes/instances
  (nested containers); custom/image views: **placeholder box only** (signed off Q4,
  2026-11-18) — the box shows the kind, the stored definition or URL as text, and an
  Info diagnostic; no image loading, no embedded plantuml/mermaid/d2 rendering.

## P3-D3a Parity overlays & navigation (signed off Q3, 2026-11-18; chrome is ours)

- **Diagram key/legend** `[i]`: toggleable side panel listing the view's rendered
  element/relationship styles + names (P2 legend machinery reused; `createLegend`-off
  views can still open the key locally — it's viewer-side, not a JSON write).
- **Tooltips** `[t]`: hover card (name, description, technology, tags, owner); global
  on/off toggle; native egui tooltips, no hover-delay tuning contests.
- **Interactive tag filter**: toolbar chip-list of the view's tags, include/exclude
  clicks re-render the scene. Pinned by P3-V7 (**CLOSED 2026-11-19**: reference is
  session-only — the toolbar mutates a JS session object, never the stored filter):
  ours is session-only too, **never** touching the model file (editor rule, P3-D0);
  include-side matching reuses `core::filter::eval`.
- **Minimap navigator**: shrunken scene overview with viewport rect, click/drag to
  jump; toggleable; cheap because the Scene is retained (re-rasterize at ≤ 1 fps).
- **Quick navigation**: type-ahead element search across the current view (name /
  description / technology substring, deterministic order) → Enter selects + centres.
- **Export** (Q2): current view **with key appended** → SVG (hand-rolled writer off
  the Scene — polylines/rects/text, no epaint involved) and PNG (same scene through
  `tiny-skia`, one code path). Deterministic SVG → expect-test snapshot; PNG QA =
  decodes + dimensions golden, no byte parity (encoder churn isn't a contract).

## P3-D4 Editing model: overlay + commands, immutable workspace untouched

- The editor holds a `LayoutOverlay` (id → geometry deltas) on top of the read-only
  `Workspace`. All edits are invertible commands (`MoveElement`, `SetWaypoints`,
  `InsertWaypoint`, `DeleteWaypoint`, `SetAnchor`, `ClearViewLayout`, grouped
  multi-select moves, plus the signed-off parity set: `AlignSelection` l/c/r/t/m/b,
  `DistributeSelection` h/v, `CenterViewOrSelection`, `SetCanvasSize` ±/auto (writes
  the view's stored dimensions — view-level overlay data), `BringOffCanvasBack`,
  `RemoveAllEdgeWaypoints`); `Vec` undo/redo stacks, cap 200,
  Ctrl+Z/Ctrl+Shift+Z/Ctrl+Y. The auto-layout button opens a dialog mirroring
  `auto-layout.jspf` (rank direction, generate-vertices on/off → overwrite vs keep
  waypoints, rank/node separation → P3-D5 params, P3-V4 semantics).
- Editor rule (P3-D0): commands may mutate overlay/JSON values only — the workspace
  model and any DSL text are immutable in this app.
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
- `Layered` = Sugiyama pipeline in **MVP scope for P3** (2026-11-14 rebalance — the
  polish half is P4-D13): cycle removal by DFS reversal; layering by **longest-path**
  (fixed iteration cap, no simplex refinement); ordering by **barycenter sweeps with
  deterministic tie-breaks**; coordinates by a **simple priority pass** honoring
  separations; orthogonal edge routing along rank gaps; **boundaries contracted naively**
  (treated as opaque super-nodes, unpacked after ordering — no interplay with ranking).
  Quality bar: "usable, stable, deterministic C4 layouts", explicitly not Graphviz
  parity. P4 upgrades: longest-path+tightening/simplex refinement, priority-method
  coordinates, smarter boundary contraction, edge-channel routing.
  `rankDirection` maps `automaticLayout` LeftRight/RightLeft/TopDown/BottomUp;
  separations from view `automaticLayout`.
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

- **M0** prep: decide reconcile home (§P3-D1: stays in api — relocation cancelled by
  the 2026-11-19 premise correction); `oxidizr-layout`/`oxidizr-viewer` scaffold; CI
  builds viewer on 3 OSes headless; `core::filter::eval` **is needed** (verified
  2026-11-19: nothing evaluates `FilteredView` mode+tags anywhere — JSON stores them
  and the reference evaluates client-side) — lands gated on P3-V5. Decomposition:
  §P3-M0 below.
- **M1** read-only viewer: open DSL/JSON, scene build, pan/zoom/select, all view
  kinds render with full styling (custom/image = placeholder boxes, Q4), light/dark,
  bundled font, icons (PNG+SVG), + parity overlays: key/legend, tooltips, tag filter
  (P3-D3a; P3-V7 checked before the persistence choice lands).
- **M2** `Layered` engine **MVP scope** (P3-D5): pipeline complete, determinism
  property, benches + committed baseline (P3 joins the milestone baseline list),
  auto-layout buttons + dialog. Quality-polish items are logged for P4-D13, not
  blockers here.
- **M3** editing: overlay + commands, undo/redo, snap, waypoint editing, the parity
  command set (align/distribute/centre, canvas size, bring-back, remove-all-vertices),
  save → byte-compatible JSON; oracle round-trip check green.
- **M4** live reload + remap UX; DSL→sidecar-layout workflow; minimap + quick-nav;
  **export** SVG+PNG with key (P3-D3a); `dot-backend` feature (optional) + docs.
- **M5** hardening: perf pass against big-bank-plc/spring-petclinic/aws corpus
  (scene rebuild + paint budget: 60 fps at 200 visible elements — measured with
  `eframe` stats overlay, informational), docs, tag v0.3.0.

## P3-M0 decomposition (charter: >4-module milestone; written 2026-11-19 before code)

Each row = one green commit unless stated; ritual = quartet + full tier + bless
no-op (byte paths untouched, but the workspace test surface grows). `oracle verify`
NOT an M0 gate (no byte-path change); V-R1-style bench sanity at close.

| # | Deliverable | Facts driving it | Exit |
|---|---|---|---|
| **M0-1** | Reconcile decision recorded (this file, §P3-D1) — **no code move** | ratified 2026-11-19: engine already `oxidizr-api::reconcile` (pub, CLI-shared), core's LOCKED P1-D1 zero-dep promise blocks the move, viewer gains the `api` dep anyway | docs commit (this one) |
| **M0-2** | `oxidizr-layout` crate: workspace member, **zero deps** (std only), neutral graph surface per §P3-D5 — `LayoutGraph { nodes, edges, rank_direction, rank_separation, node_separation }`, `LayoutResult`, `trait LayoutEngine`; `Layered`/`Dot` impls are M2/M4 — here: types + trait + doc + unit tests for the types' invariants | P3-D5; `cargo xtask ci` covers new members automatically (workspace-wide) | quartet green |
| **M0-3** | `oxidizr-viewer` crate: lib core modules `scene`/`hit`/`commands`/`reload` as compiling stubs importing NO eframe (P3-D1 boundary), thin `main.rs` eframe bin; deps core+api+json+layout + eframe (wgpu)/notify/usvg/tiny-skia/dark-light; version pins recorded in the crate's `DEPENDENCIES` rustdoc (P3-D2); `cargo add` works — network verified 2026-11-19 (toolbox.md corrected) | registry cache lacks the GUI stack → first fetch pulls; `unused_dependencies` lint is allowed (P1-D9 pattern) so declaring the agreed set early is safe | quartet + full tier (workspace test surface grows) |
| **M0-4** | CI headless builds: existing check/clippy jobs are workspace-wide so the new members ride for free — verify argv; extend `cargo xtask ci windows` leg check for viewer (eframe/winit/wgpu on `x86_64-pc-windows-gnu` mingw — pure-Rust bindings expected); darwin stays CI-only | P3-D8 "CI builds but runs no GUI"; no xvfb | quartet + `xtask ci windows` |
| **M0-5** | `core::filter::eval` — pure evaluator `(&Workspace, base view content, mode, tags) → included element/relationship sets` over core types only (core stays dependency-free ✓); **implement only after P3-V5** settles reference semantics (include=any-tag, exclude semantics, relationship keep-rule incl. endpoint survival + `directRelationshipsOnly` corners) against the pinned monorepo/`filter.jspf`; unit tests incl. determinism; NOT wired into the JSON writer (bytes contract untouched — the writer stores mode+tags as today, matching the golden) | verified: `create_filtered_view` stores mode+tags; writer emits them un-evaluated; no evaluator exists | **DONE 2026-11-19** — V5 verdict above; `crates/oxidizr-core/src/filter.rs` (`eval` + `Included`, order-preserving, cycle-guarded chain walk, stale-id rule documented) + 8 unit tests; writer untouched |
| **M0-6** | AGENTS.md one-sentence clarification: `unsafe` rule covers oxidizr code, dependency-internal unsafe (wgpu) is out of scope (P3-D2) | charter touch-up, no behavior | with M0-3 or its own docs commit |
| **M0 close** | PROGRESS board + log M0 row; PHASE_3 M0 DONE; RESUME → M1 entry point; quick `bench cmp 2R` sanity (engine untouched) | — | **DONE 2026-11-19** — docs landed; `bench cmp 2R` all groups in noise (one `m7_json_write_corpus` 1.15 outlier re-measured at 1.80 ms = −15% vs baseline ⇒ run noise, not `filter.rs`); quartet + full tier + bless corpus-diff=0 |

Order: M0-1 → M0-2 → M0-3 (+M0-6) → M0-4 → M0-5 → close. M0-5 may parallel M1 work
but its V-item gate is hard.

## P3-M1 decomposition (charter: >4-module milestone; written 2026-11-19 before code)

Read-only viewer (§M1). Gating V-items all CLOSED 2026-11-19: V2 (bundled font
substitution confirmed — the reference never loads web fonts at all), V3 (icons =
PNG-only corpus → atlas), V5 (filter semantics → `core::filter::eval`), V7 (tag
filter session-only). Boundary guard: it forbids `eframe`/`egui` tokens in lib
sources; **`epaint` is allowed** (pure-Rust, headless) — the scene keeps the P3-D3
promise using `epaint::LayoutJob` text runs with no shell dependency.

| Item | Deliverable | Premise / verified | Exit gate |
| --- | --- | --- | --- |
| **M1-1** | `oxidizr-api::open(input, &ApiOptions) -> Opened { workspace, log }` — public entry mirroring the `dsl2json`/`json2json` workspace build (read → parse: DSL partial-on-error OK, JSON strict → `apply_default_views` → validate), errors as values, `execute`/CLI untouched | pipeline stages exist but are private (`read_input`/`parse_dsl_input`/`json_loader`, pipeline.rs:274-420) | **DONE 2026-11-19** — `src/open.rs` (+4 helpers `pub(crate)`); DSL statement errors open PARTIALLY (viewer twist, documented), validate tail informational; 7 integration tests (`tests/open.rs`); `execute` untouched, bless no-op |
| **M1-2** | `scene` core (headless): `Scene { view_key, title, background, boundaries, items (rect + text runs), edges (polyline + label), bounds }` + `build(&Workspace, view_key, ColorScheme, Option<&Included>)`; geometry = stored `x/y` (read-only M1 — no auto-layout yet); **deviation from the 2026-11-19 sketch: text kept as plain `String` runs — no `epaint` dep, layout jobs deferred to M1-4 paint** (lib core stays egui-stack-free, guard unchanged); styles via `core::styles` resolution (P2-D2), light + dark schemes; deterministic (same input → identical scene) | P3-D3; `thiserror` added (viewer lib) for `SceneError` — 4 un-renderable-view variants, stale ids skipped never errors; DN = boundary not item (diagram.js:558); boundary kinds group / deployment / parent-SS / parent-container, rects = member unions + padding; filtered views resolve inside `build`, session `mask` INTERSECTS the view's own filter (V5/V7) | **DONE 2026-11-20** — `src/scene.rs` (~1850 lines) + 14 unit tests incl. determinism; quartet + full tier 8/8 + bless diff 0 + guard green |
| **M1-3** | `hit` core (headless): `Viewport` transform (world↔screen, zoom clamp) + AABB element hit + point-to-segment edge hit; unit tests incl. clamp bounds + degenerate segments | hand-rolled geometry per P3-D3 (no geom crate); clamps/tolerance are OURS (visual parity out of contract, P3-D0): `MIN/MAX_SCALE` 0.05/20 (NaN-safe), `HIT_TOLERANCE_PX` 6 screen px ÷ scale, `Viewport::fit` + `FIT_PADDING_PX` 40; `zoom_at` keeps the pivot world point fixed; pick: last item wins (paint order), then FIRST edge within tolerance (content order); boundaries not pickable until M3 | **DONE 2026-11-20** — `src/hit.rs` + 10 unit tests; quartet + full tier 8/8 + bless diff 0 + gui-leg clippy clean |
| **M1-4** | gui shell render + navigate: open via argv/menu (`File → Open`), view list, canvas pan/zoom/select wired to `hit`, full styling paint (fill/stroke/dash/opacity/font size, group labels), all view kinds incl. custom/image → placeholder boxes (Q4), light/dark toggle + `dark-light` startup detection, tooltips, key/legend panel, tag-filter chips (session-only per V7; include/exclude via `core::filter::eval`) | **split 2026-11-20 at the canvas-vs-overlays seam: M1-4a = new headless `paint` module + shell (open argv/toolbar, view list, pan/zoom/select, element-URL ctrl-click, tooltips, light/dark + `dark-light` startup, Fit, eframe-storage persistence) — DONE 2026-11-20; M1-4b = key/legend panel, tag-filter chips, diagnostics panel — DONE 2026-11-20.** Seam decision recorded: `epaint` = unconditional viewer-LIB dep (pure software raster/layout, the M1-2 sketch always meant this); `paint::paint_scene(&Scene, &Viewport, &mut FontsView, &PaintOptions) -> Vec<Shape>` returns data, the shell draws it verbatim → whole paint path is Category A; guard stays eframe/egui-only (7 lib files since M1-4b). `clap` rides the `gui` feature. Opacity = per-color alpha, dashes via `Shape::dashed_line_many` (epaint 0.37 has no dashed stroke style), glyphs = polygon approximations, arrows = destination triangles — visual-parity approximations, out of contract (P3-D0). Integration test caught box text anchoring LEFT at centre-x → `Align2::CENTER_TOP` (labels were overflowing right); `tests/golden_paint.rs` guards open→build→paint headlessly | eframe-0.37 traps logged: unified `egui::Panel` (no `TopBottomPanel`/`SidePanel`), `Ui::scope_id`, `Id::unique`, `allocate_rect(rect, Sense)`, `Storage::set_string(… String)`, epaint 0.37 = `text::Fonts` (no `FontAtlas`), no `rect()` fn, `line_segment([Pos2;2], stroke)` | **M1-4a DONE 2026-11-20** — `src/paint.rs` (~780 lines, 7 unit tests) + shell rewrite + 3 golden-paint tests; quartet + full tier 8/8 + bless diff 0 + `ci gui` green. **M1-4b DONE 2026-11-20** — new headless `key` module (`legend` folds a Scene into deduped element/edge/boundary key rows by first appearance; `tags` = stored-tag inventory w/ counts; `session_mask` = chips → `filter::eval` on the view's RENDER BASE content per V5/V7 — session-only, never persisted, intersects the view's own filter) + 5 unit tests; shell: key panel (right, swatches via painter), tag chips under the view list (exclude/include mode + clear, rebuild on change), diagnostics WINDOW from the status-bar count (severity-colored rows, `oxidizr-diag` dep for the `Severity` enum). Trap logged: `filter::eval` Include keeps a relationship ONLY when the RELATIONSHIP's own tags hit (endpoint survival is not enough) — fixture now tags the rel to pin that |
| **M1-5** | icons: PNG decode + foreign SVGs rasterized; one texture atlas; missing/broken → placeholder box; element icon slots + key rows | **DONE 2026-11-20.** New headless `icons` module: sources per D10 (data URIs base64/percent, file paths vs the workspace file's `base_dir`, `http(s)` NEVER fetched — offline posture; foreign bytes → `Unsupported`); PNG via `png` 0.18 to RGBA8 (`ALPHA\|EXPAND\|STRIP_16` + manual Rgb/Grayscale expansion); SVG seam decision recorded: **`resvg` 0.50** (`usvg` 0.50 + `tiny-skia` 0.12 trio, published 2026-11-11) — tiny-skia 0.12 REMOVED the 0.11 usvg `Renderer` (grep-verified), resvg is the sanctioned seam; **zero default features** on usvg/resvg (no fontdb/harfrust/system-fonts) → SVG-with-text renders text-less, the §P3-D7 limitation V3 made moot; one atlas keyed (icon, size, scheme), deterministic shelf pack, `ICON_RASTER_PX` 64, oversized PNGs box-scaled, failures once each in `Atlas::errors`. Shell: ONE `load_texture` upload (skipped on identical pixels), cells as uv rects (½-px inset vs LINEAR bleed) overlaid after `paint_scene` via new pub `paint::icon_slot`/`to_screen_rect` (shared with the placeholder geometry — placeholder stays for failed icons), key-row chips, diagnostics rows, status count = open-log + icon errors; `ElementKey` gained `icon` (quintuple dedup); guard now 8 lib files. 8 unit tests | V3 CLOSED: bundled themes are 100% PNG, zero SVG-with-text | quartet + `cargo xtask ci gui` + full tier 8/8 + bless 307×unchanged — all green 2026-11-20 |
| **M1 close** | PROGRESS board + log M1 row; PHASE_3 M1 DONE; RESUME → M2 entry point | — | **DONE 2026-11-20** — docs landed (M1 log in PROGRESS + this table + RESUME → M2); quartet green on the docs commit; bench N/A — engine untouched since `410fee6`, bless diff 0 held every M1 commit. **§P3-M1: M1 COMPLETE (`969a31b`→`496912c`)** |

Order: M1-1 → M1-2 → M1-3 → M1-4 → M1-5 → close. M1-4 is the big-bang commit; if it
drags, split at the canvas-vs-overlays seam (each half still gates green).

## P3-M2 decomposition (charter: >4-module milestone; written 2026-11-20 before code)

`Layered` engine MVP (§P3-D5; P3-D9 M2 bullet): pipeline complete,
determinism property, benches + committed `P3` baseline, viewer
auto-layout buttons + dialog. V1 CLOSED (§P3-D10) drives the tuning.
The engine consumes the M0-2 neutral graph (`LayoutGraph`/
`LayoutConfig`/`trait LayoutEngine`, ZERO deps) and knows NOTHING of
boundaries: the viewer contracts them by construction — boundary
elements are not layout nodes, the scene recomputes boundary rects
from member unions (scene.rs), which IS the naive super-node contract
with the V1 50 px pad. Layout is NOT persisted in M2 — session
geometry only (M3 owns save); byte contract untouched all milestone.

| Item | Deliverable | Premise / verified | Exit gate |
|---|---|---|---|
| **M2-1** | docs: §P3-M2 decomposition (this table) + P3-V1 CLOSED verdict | V1: dagre 1.1.8 client layout; dialog defaults LR/100/50/50 + vertices = core defaults; cluster padding 50; no per-view-type direction | docs commit |
| **M2-2** | `oxidizr-layout::Layered` pipeline half 1: prep (graph-order stable ids) + cycle removal (DFS reversal, first-edge tie-breaks) + layering (longest-path, iteration cap) + long-edge dummies; unit tests (cycle, diamond, chain, disconnected, cap) | **DONE 2026-11-20 — DEVIATION recorded: landed the WHOLE pipeline end-to-end** instead of half 1 only — `impl LayoutEngine for Layered` (was M2-3's) plus naive stage 4 (initial order: reals by node index, dummies by edge order) and stage 5 (stacked coords: ranks advance by rank-max extent + `rank_separation`, cross-stack + `node_separation`, centred on the mid-line; canonical frame + LR/RL/TD/BU mirroring) and stage 6 (anchor-aware routes: explicit `Anchor` side or dominant-delta side facing the neighbor, dummy centres when `generate_vertices`, self-loop stubs) — so every code path is live and tested NOW; M2-3 replaces stages 4/5/6's naive passes, the plumbing stays. `LayoutError::CycleRemovalFailed` added (defensive-only). 21 unit tests incl. exact-float coords + a 60-node determinism repeat | §P3-D5 MVP steps; dagre-family phases per V1 | quartet + full tier + bless (workspace test surface grows) — all green 2026-11-20 |
| **M2-3** | pipeline polish (re-scoped after M2-2's deviation — the engine seam + routes already exist): REPLACE stage 4 with barycenter sweeps (FIXED count, deterministic tie-breaks) and stage 5 with the priority coordinate pass honoring rank/node separation; orthogonal-ish edge routing along rank gaps; determinism property test up to 1k-node synthetic | **DONE 2026-11-20** — stage 4: 4 rounds × (down + up) cascading barycenter sweeps, `total_cmp` means with current-position tie-break, best-of-rounds by exhaustive crossing count (ties keep the EARLIER order); stage 5: 4 alternating-side priority passes — dummies first so chains straighten, moves clamped to the slot the within-rank neighbours leave open (order + `node_separation` preserved by construction; over-constrained resolves to the upper bound), final shift returns the lowest box to the zero line; stage 6: diagonal legs orthogonalised through mid-rank-coordinate channels (rank GAPS; mid-cross channel when both ends pin cross sides), straight runs + zero-length steps collapsed; `generate_vertices=false` still emits the straight 2-point route. New tests: bipartite untangle (exact), 4-direction axis-alignment, 1k-node determinism + finiteness; the 3 position/waypoint pins re-derived exactly (a/c centre 70, dummy line 110) | §P3-D5 determinism hard contract (no randomness, stable sorts, fixed iterations) | quartet + full tier 8/8 + bless 307×unchanged — all green 2026-11-20 |
| **M2-4** | benches: criterion targets in `oxidizr-layout` (`harness = false` per the `reconcile` pattern) — 1k-node synthetic + fan/chain shapes; committed as the `P3` baseline at close | **DONE 2026-11-20** — `benches/layered.rs`: `p3_layered_thousand_{mixed,chain,star}` (mixed = stride edges + back edges every 11th → cycles+dummies; chain = 1k ranks; star = one rank pair with 999 edges — the O(e²) crossing-count worst case). `criterion` as a DEV-dependency only — the §P3-D1 zero-dep promise governs `[dependencies]` (recorded in the manifest comment). Measured (this box): mixed 3.7 ms, chain 0.71 ms, star 0.97 ms per 1k-node layout | §P3-D5 bench mandate; P3 joins the milestone baseline list (P3-D9 M2 bullet) | quartet + bench run (+ full tier 8/8 + bless 307× — all green 2026-11-20) |
| **M2-5** | viewer integration: new headless `layout` module (scene→`LayoutGraph`, engine run, `Overrides { element rects, edge waypoints }`) + `scene::build` gains `Option<&Overrides>` (session-live, NEVER persisted, V7-style stance); auto-layout BUTTON + dialog mirroring `auto-layout.jspf` (direction, vertices, rank/node/edge separation; localStorage→our eframe-storage); views with a STORED `automaticLayout` get ONE engine pass per (view, params) at open; boundaries follow via member unions + 50 px pad | **DONE 2026-11-20** — `layout.rs`: `stored_for` (filtered→base resolution), `config_for` (null direction ⇒ LR), idempotent `graph_for` (ids + style-resolved sizes ONLY — no positions, so a re-run on a laid-out scene reproduces identical overrides), `apply` (placements → element top-lefts; route `points[1..len-1]` → interior waypoints; empty vec straightens), `encode`/`decode` (params persist under `oxidizr.viewer/layout`). `scene::build` 5th param `Option<&Overrides>`: element top-left replaces stored `x/y`, relationship interiors REPLACE stored vertices; bounds recompute; guard 8→9 files. Shell: `Layout` toolbar button + `Automatic layout` window (Apply/Reset), ONE pass at adopt/view-change for views with stored `automaticLayout`, params persisted, overrides session-only + cleared on view change. **DEVIATION (pad): boundaries keep `BOUNDARY_PADDING = 20` — the M1 chrome constant — NOT V1's dagre-internal 50 px: the reference applies that pad inside its generator, ours re-derives rects from the existing union rule, so the viewer stays self-consistent with stored-geometry rendering.** 7 new `layout.rs` tests + 3 `scene.rs` override tests | V1 (dialog = stored `AutomaticLayout`, oxidizr-core mirror verified); scene seam = `element.x/y` rect construction (scene.rs ~487) | quartet + `ci gui` + full tier + bless — all green 2026-11-20 |
| **M2 close** | PROGRESS board + log M2 row; PHASE_3 M2 DONE; `bench save P3` + `bench cmp P3` sanity; RESUME → M3 entry point | **DONE 2026-11-20** — PROGRESS board + P3-M2 log row; this table's V1 row pad wording corrected to match the M2-5 deviation (20 px chrome pad, not dagre-internal 50); `bench save P3` baseline committed (104 files, incl. the three `p3_layered_thousand_*`); `bench cmp P3` all groups 1.00 (chain 729 µs / mixed 3.9 ms / star 999 µs); RESUME → M3 (editing + save, V4 gate) | full ritual | 

Order: M2-1 → M2-2 → M2-3 → M2-4 → M2-5 → close. Engine internals stay
module-private: the public surface remains `Layered` + the M0-2 types.

## P3-M3 decomposition (charter: >4-module milestone; written 2026-11-20 before code)

Editing + save (§M3 = P3-D4 + the P3-D9 M3 bullet): per-view
[`LayoutOverlay`] over the read-only workspace, invertible commands +
undo/redo, drag/snap, waypoint editing, the parity command set, and
SAVE through the existing `oxidizr-json` writer (byte contract: the
save path adds NO serializer; WRITE-CATALOG rules apply unchanged).
V4 CLOSED (§P3-D10): stored `automaticLayout` blocks are honoured,
permanent and never block editing here — the overlay wins the session,
save writes GEOMETRY ONLY. The M2 `scene::Overrides` IS the overlay's
geometry payload: `commands` graduates the M0 stub, the shell replaces
`layout_overrides` with the per-view overlay map (session state, kept
across view switches like the reference's in-memory JSON, cleared at
open/reload). Scope cuts recorded up front: **`SetAnchor` and
width/height deltas are P4** (the P3-D9 M3 bullet lists the parity set
WITHOUT anchors; item sizes are style-resolved, V1-adjacent — the
overlay carries element TOP-LEFTS, relationship interiors and the view
`dimensions` only).

| Item | Deliverable | Premise / verified | Exit gate |
|---|---|---|---|
| **M3-1** | docs: this table + the P3-V4 CLOSED verdict | V4: the reference NEVER writes the block (zero `setAutomaticLayout` call sites), re-lays-out every render and locks non-editable (`structurizr-diagram.js:355/:400/:989`) — we keep editing ON, block permanent, save = geometry only; dialog houses Bring-back + Remove-vertices (auto-layout.jspf) | **DONE 2026-11-20** `4fcb235` — docs commit |
| **M3-2** | headless `commands` module: `LayoutOverlay { elements (top-lefts), relationships (interiors), dimensions }` + the invertible `Command` set (`MoveElements` group, `SetWaypoints`, `ClearView` snapshot, `SetCanvasSize`) + FACTORIES for the parity set (align l/c/r/t/m/b, distribute h/v, centre, bring-back, remove-all-vertices — each computes its `MoveElements`/`SetWaypoints` delta from the overlay + scene rects) + `History` (Vec undo/redo, cap 200, apply/undo symmetric) | commands mutate ONLY the overlay (editor rule, P3-D0); inverse = swap from/to, no stored closures; grid snap 10 px lives here (`snap_grid` pure fn); `as_overrides()` feeds `scene::build`; unit tests: every command round-trips (apply→undo = identity), cap drops oldest, factories against fixture rects | **DONE 2026-11-20** `6b401c0` — `LayoutOverlay { view_key, elements, relationships, dimensions }` (readers `position`/`rect`/`interior` = overlay-first-then-scene), `Command { MoveElements{moves}, SetWaypoints{changes}, SetCanvasSize{from,to}, ClearView{snapshot}, Group{steps} }` — symmetric `apply(forward)`, no closures; factories `move_by` (`snapped`, GRID_PX 10), `align`/`distribute`/`centre_view`/`bring_back`/`remove_all_vertices`/`set_waypoints`/`set_canvas_size`/`clear_view`/`apply_layout` — ALL `Option<Command>`, `None` = no-op never pushed; `History` run/undo/redo/reset, HISTORY_CAP 200. **Undo semantics pinned: undo REWRITES `from` geometry but map ENTRIES persist — round-trip identity is at RENDER level (the readers), not map equality.** quartet + full tier + bless green |
| **M3-3** | shell interactions: left-drag ELEMENT moves (snap toggle, group-move keeps deltas), left-drag EMPTY canvas box-select (+shift add), pan moves to middle-button/space per P3-D4, click select (paint-order pick), Ctrl+Z/Ctrl+Shift+Z/Ctrl+Y wired to `History`, per-view overlay map (`HashMap<view_key, LayoutOverlay>`), dirty badge, auto-layout Apply/Reset land as COMMANDS (undoable) | shell keeps ZERO geometry math (factories + `hit::items_in_rect` headless helper — new); one-pass `autolayout_once` unchanged, writes the overlay; rebuild on every command (cheap, M1-D3 scene-cache contract intact) | **DONE 2026-11-20** `c0ad59e` — canvas state machine `Drag { Idle, Pan, Move{origins,press,delta}, Lasso{anchor,base} }`: MODE decided AT PRESS (`press_mode`), preview frames write `drag_target(origin, delta, snap)` into the overlay so they EQUAL the committed geometry, release commits the press-captured `drag_move` (idempotent re-apply; `None` restores the press positions); `hit::items_in_rect` intersection lasso (+Shift extends the press-time base); Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y per-view (`path_focus` guard); per-view `overlays` + `histories` maps + `dirty: BTreeSet` badge in the view list — overlays survive VIEW switches, cleared at adopt (scope cut); Apply/Reset → `apply_layout`/`clear_view` commands; Snap toolbar toggle. quartet + `ci gui` + full tier + bless green |
| **M3-4** | waypoint editing + parity UI: selected edge draws its waypoint handles; drag handle / double-click insert / Alt+click delete → `SetWaypoints`; dialog gains the two reference-footer buttons (magnet Bring-back, Remove-vertices) + align/distribute row for multi-select; empty-waypoint contract = straight (M2-5 `Overrides` semantics reused verbatim) | `hit` gains `nearest_waypoint`/`insert_waypoint` pure helpers (segment distance reuses the edge-hit math); handle painting rides `paint::paint_scene` (Category A) | **DONE 2026-11-20** `6c5d541` — `hit::nearest_waypoint` (nearest INTERIOR vertex within tol world px — endpoints belong to the boxes) + `hit::insert_waypoint` (splice index splitting the nearest segment); `commands::drag_waypoint` keeps the press-captured contract; `paint::WAYPOINT_HANDLE_PX = 4.0` filled+ring circles on the selected `Selection::Edge`; shell `Drag::Waypoint`, `press_mode(origin, shift, alt)` grabs handles of the SELECTED edge FIRST (Alt+press deletes a vertex, plain press drags), double-click splices via `insert_waypoint_at`; dialog footer = Align L/Cx/R/T/Cy/B (≥2), Space ↔/↕ (≥3), Centre, Bring back (visible canvas as world page rect), Remove vertices — all through `run_edit` → one undoable step. quartet + `ci gui` + full tier + bless green (lib 69 tests) |
| **M3-5** | SAVE: headless `apply_overlay(&Workspace, view_key, &LayoutOverlay) -> Workspace` (elementViews x/y, relationshipViews vertices, view `dimensions` — clone-and-write, model untouched) + `oxidizr_json::to_vec` + temp+rename write (P3-D6 posture); Ctrl+S + toolbar Save; target = opened `.json`, sidecar `<stem>.json` for DSL sources; Save blocked (status note) on filtered views | Category A: apply→write→`from_slice` round-trip equals the in-memory scene; bless 307×unchanged proves the writer path byte-neutral; oracle `json` load of a saved sidecar is the M5 A/B leg (needs `$OXIDZR_STRUCTURIZR_BIN`) | **DONE 2026-11-20** `1d58964` — `apply_overlay` clone-and-writes `x`/`y`, vertices (wholesale, empty straightens), `dimensions` (set only when the overlay carries one) through the 7 MODEL-view vecs by `base.key` — filtered/image views have NO content slot, the key lands nowhere and the shell BLOCKS their save with a status note; `to_px` rounds+clamps to the stored i32 grid; model/styles/`automaticLayout` untouched (V4). `save_target`: `.json` in place, `.dsl` → `<stem>.json` sidecar (case-insensitive), `-` stdin → None; `write_workspace` = shared `oxidizr_json::to_vec` → `.{pid}.tmp` → rename, `SaveError { Json, Io{path,source} }` (no second serializer). Shell: Ctrl+S (focus-guarded) + toolbar Save (enabled while dirty), status-bar note, success clears `dirty`. Category A test asserts the round-trip as BYTE STABILITY (`to_vec(from_slice(bytes)) == bytes`) — the JSON read applies the reference bean normalizations (`Some("")` descriptions, tag rebuild) and the writer sorts entry arrays by id, which is exactly what makes repeated saves idempotent. quartet + `ci gui` + full tier + bless green (lib 71 tests) |
| **M3 close** | PROGRESS board + log M3 row; PHASE_3 M3 DONE; RESUME → M4 entry point; bench sanity (engine untouched → `cmp P3` spot-check only) | — | **DONE 2026-11-20** — docs landed (M3 log in PROGRESS + this table + RESUME → M4); `bench cmp P3` all groups 1.00 (chain 729 µs / mixed 3.9 ms / star 999 µs — engine untouched all M3, viewer-only work); **§P3-M3: M3 COMPLETE (`4fcb235`→this commit)** | full ritual |

Order: M3-1 → M3-2 → M3-3 → M3-4 → M3-5 → close. If M3-3 drags, split
at the drag-vs-box-select seam (select+undo first, drag second), each
half gating green. Undo history is per-view (switching views switches
history — the reference's per-diagram undoStack behaviour).

## §P3-M4 — live reload, navigation, export (decomposition, 2026-11-20)

M4 = the P3-D9 M4 bullet: live reload + remap UX (P3-D6), DSL→sidecar
workflow, minimap + quick-nav, SVG/PNG export with key (Q2),
`dot-backend` feature. V6 CLOSED for the Linux leg (probe, below) —
the design (parent-dir watch + path filter + debounce + content-hash)
absorbs whatever the other OSes emit; the Windows/macOS legs stay an
open CI observation, not a blocker. V8 stays informational (P3-D0: no
pixel parity): we compose the key BELOW the diagram with margins,
which is what `export.jspf` does — no further look needed.

| Item | Deliverable | Premise / verified | Exit gate |
|---|---|---|---|
| **M4-0** | docs: this table + the P3-V6 Linux verdict below | probe: `notify` 8 `recommended_watcher` on Linux = inotify on a DIRECTORY (file watches die on rename-swap); facts + posture recorded below  **DONE 2026-11-20** `81b0a3d` — docs commit |
| **M4-1** | file-set plumbing + headless `reload` core: `ParseResult.includes: Vec<String>` (every file-system `!include` resolved file, in parse order; URL includes are policy-dead so the set is FS-only), `Opened.includes` (empty for JSON/stdin), and `ReloadDriver` in `reload.rs`: watch PARENT dirs (NonRecursive) of the display file + sidecar + includes, path filter (exact watched paths, `.{pid}.tmp` never matches), raw events → channel, 250 ms settle-debounce per path-set, content-hash compare per path (P3-D6 self-save rule: our rename IS an external-looking `Modify(Name(To))` — indistinguishable without the hash) → `Change` signal | probe facts (M4-0); `read_files` resolves file/dir/glob includes at parse time (`stmt_line.rs:194`); driver headless — notify is already an un-gated viewer dep (`reload` M0 stub says so); R-2 guard untouched  **DONE 2026-11-20** `60fbdc4` — quartet + full tier + bless green; watch set + debounce + hash authority shipped |
| **M4-2** | shell reload wiring + remap UX: `egui::RepaintSignal` wakes the app; reload = `open(new bytes)` → `oxidizr_api::reconcile(layout, updated)` where `layout` = current workspace + dirty overlays folded; overlays re-keyed by the remap (ambiguous = conservative drop + report, R-3), histories reset, last-good workspace stays visible when the reparse fails (parse regressions → diagnostics panel, window stays open); status badge added/removed/ambiguous counts | `reconcile(layout, updated) -> (Workspace, Report)` is the SHARED CLI engine (`oxidizr-api::reconcile`); adopt already canonicalizes display; after a reload `dirty` is empty (geometry baked)  **DONE 2026-11-20** `4439931` — quartet + `ci gui` + full tier + bless green; remap re-keys overlays, last-good holds on bad reparse |
| **M4-3** | DSL→sidecar workflow: once a sidecar exists it becomes the preferred reload source AND the open-priority target (save already writes it — M3-5); watch set refreshes on save; docs note in the help overlay | M3-5 `save_target` + byte-stability test; P3-D9 M4 bullet  **DONE 2026-11-20** `cba1f4b` — quartet + `ci gui` + full tier + bless green; newest-wins, ties→sidecar |
| **M4-4** | minimap (navigator.jspf analogue) + quick-nav: headless `overview.rs` — `overview(&Scene, box px) -> Vec<Shape>` (boxes as dots/rects, viewport rect overlay) painted by the shell bottom-right; click/drag the map centers the viewport; quick-nav = search field (elements by name/id + view titles) → center + select | `paint::paint_scene` subset reuse (Category A, same shape-list contract); `hit::Viewport` already does world↔screen  **DONE 2026-11-20** `e33c675` — quartet + `ci gui` + full tier + bless green; `overview.rs` + `Drag::Minimap` + nav search |
| **M4-5** | export SVG+PNG with key: headless `export.rs` — `shapes_to_svg(shapes, size)` translator over OUR shape subset (Line/Path(Open|Closed)/Rect/Circle/Ellipse/Text; no bezier/gradient/image shapes are emitted by `paint`); text = glyph OUTLINES as paths: decode the SAME base64 font data epaint embeds (`FontDefinitions.font_data` + `base64` dep, already declared) → `ab_glyph` (epaint's own rasterizer ⇒ version-unified) `Outline` points → quadratic path, per-galley position/color/size; PNG = our own SVG → `resvg` → tiny-skia pixmap → `png` encode (the pinned trio rides along; no font feature ⇒ hence the outline decision); key section composed BELOW the diagram (V8 anatomy) reusing the `key::legend` rows | seam-verified 2026-11-20: epaint 0.37 has NO svg serializer (we write our own), `Tessellator::tessellate_shape` exists (PNG fallback if resvg path stalls), ab_glyph IS epaint's glyph crate; files `<stem>.svg/.png` next to the save target; dialog-free: toolbar Export  **DONE 2026-11-20** `ca4221f` — quartet + `ci gui` + full tier + bless green; snapshot gate = usvg-parse + PNG magic (P3-D0 held, bytes NOT goldens) |
| **M4-6** | `dot-backend` feature (optional): `oxidizr-layout::Dot` engine (P3-D5 neutral trait) shelling to `$OXIDZR_DOT_BIN` (`-Tplain` parse → placements), off by default, Category B test gated on the env var (oracle posture); viewer dialog engine combo lists it only when available | R-1 mitigation names this row; `LayoutEngine` trait exists since M0-2; `Layered` stays the default  **DONE 2026-11-20** `dc5d840` — quartet + gated Cat-B test green on graphviz 15.1.1; `-Tplain` format reality vs the man page recorded in `dot.rs` docs |
| **M4 close** | PROGRESS board + log M4 row; PHASE_3 M4 DONE; RESUME → M5 entry point | — | **DONE 2026-11-20** — docs landed (M4 log in PROGRESS + this table + RESUME → M5); full ritual green; **§P3-M4: M4 COMPLETE (`81b0a3d`→this commit)** |

Order: M4-0 → M4-1 → M4-2 → M4-3 → M4-4 → M4-5 → M4-6 → close. Split
rules: M4-5 may ship SVG first (PNG rides the same translator); if
ab_glyph outline extraction stalls, the recorded fallback is SVG
`<text>` + an embedded base64 `@font-face` (same font bytes, less
portable viewers) — decision stays INSIDE M4-5, no scope moves.

P3-V6 verdict (Linux leg, CLOSED 2026-11-20, probe = scratch crate,
notify 8.x, inotify backend, 400 ms drain windows): a plain
`fs::write` fires `Access(Open)` + `Modify(Data)` +
`Access(Close(Write))` (same-content rewrites are indistinguishable ⇒
content-hash compare is the ONLY dedup); our save posture (temp write
+ rename) fires 4 temp-path events + `Modify(Name(From/To/Both))`
where the `To` path IS the target — i.e. an atomic external save and
ours are EVENT-IDENTICAL, only the hash filter separates them;
delete→recreate arrives as `Remove` then `Create` (watching the FILE
would die on this — watching the PARENT DIR survives, hence
NonRecursive dir watches + exact-path filter); 5 rapid writes = 13
raw events ⇒ the 250 ms per-path-set settle-debounce is mandatory.
Windows/macOS coalescing + rename semantics: CI-observed, not
blocking (the filter+debounce+hash posture is OS-agnostic).

## §P3-M5 — hardening + v0.3.0 (decomposition, 2026-11-20)

M5 = the P3-D9 M5 bullet: oracle pass (incl. the saved-sidecar A/B leg
deferred from M3-5), perf pass, docs, tag. Small surface (xtask + docs
+ notes), so the table is short; no V-items open — P3-D0 keeps the
perf numbers INFORMATIONAL (no golden byte contracts here).

| Item | Deliverable | Premise / verified | Exit gate |
|---|---|---|---|
| **M5-0** | docs: this table | oracle verify core legs already green 2026-11-20 (`4 json + 90 diagram + 6 theme golden set(s) match`); perf corpora located: `big-bank-plc.dsl`, `spring-petclinic/`, `amazon-web-services.dsl` | **DONE 2026-11-20** — `5f1dd32` |
| **M5-1** | saved-sidecar A/B leg in `xtask oracle verify` (deferred from M3-5): for each top-level JSON golden fixture open via `oxidizr_api`, run the viewer save path (`commands::write_workspace`) → IDENTITY sidecar; the reference's json→json export of (a) our sidecar and (b) the committed golden must normalize BYTE-EQUAL — comparing the two re-exports isolates our writer from any reference json→json quirk. EDITED leg: `layout::apply` on the first non-filtered view with ≥ 2 items → `LayoutOverlay` → `apply_overlay` → save → reference load SUCCEEDS, `model` Value unchanged, per-id `x/y` map of our edited sidecar == the reference re-export's map, and differs from the stored one | the reference loads `.json` inputs already (the `m3b-defaults` golden's fixture IS a probe JSON — `verify_json_legs` runs it today) | **DONE 2026-11-20** — `0842dbd`; `oracle verify` green: 4 json + **4 sidecar** + 90 diagram + 6 theme — identity re-exports byte-equal, edited layered placements round-trip the reference loader verbatim |
| **M5-2** | perf pass (informational, P3-D0): headless `scene::build` + `paint::paint_scene` timings, release, n ≥ 3 per figure, corpora big-bank-plc + spring-petclinic + amazon-web-services; note ≤ 25 lines under `agents/notes/perf/` + indexed, with the 60 fps / 200-visible-elements budget called out per corpus | paint runs headless (M4-5 export rides it — `golden_paint.rs` proves the no-GPU path) | **DONE 2026-11-20** — `2325947`; probe `tests/perf_scene.rs` (ignored) + note `agents/notes/perf/scene-paint-budgets.md`: budget grid-tiled to 209 items + 247 edges ≈ 1.0 ms/frame ⇒ ~16× headroom under 16.7 ms |
| **M5-3** | docs: `crates/oxidizr-viewer/README.md` (open/save/sidecar rule, edit + auto-layout + engines, export, shortcuts, feature flags incl. `dot-backend` + `$OXIDZR_DOT_BIN`); check `docs/diagnostics.md` mentions nothing false about the viewer | viewer README absent today; RESUME holds the authoritative surface list | **DONE 2026-11-20** — `ff2b8b5`; README landed; `docs/diagnostics.md` checked — makes no viewer claims, nothing to fix |
| **M5 close** | PROGRESS board + log M5 row; PHASE_3 M5 DONE; RESUME → post-P3 (P4) entry point; full ritual; local tag `v0.3.0` — never push | — | **DONE 2026-11-20** — docs landed (M5 log in PROGRESS + this table + RESUME → P4); full ritual green (quartet + `ci gui` + full tier 8/8 + bless 307× unchanged); tag `v0.3.0` local, never pushed; **§P3-M5: M5 COMPLETE — Phase 3 complete (`346ca6e`→this commit)** |

Order: M5-0 → M5-1 → M5-2 → M5-3 → close. The oracle binary is slow
(~1 s JVM start) — M5-1's legs ride the existing Category B job
posture, never the per-PR quartet.

## §P3-M6 — post-v0.3.0 follow-up: display auto-layout (2026-11-20)

User report after the tag: views of a file without `location` statements
opened piled at `(0, 0)` — "automatic layout does not work". Root cause:
the loader materializes a missing location as the stored `0`/`0` pair
(V-2), and the M2-5 open pass fired ONLY for views carrying an
`automaticLayout` statement (and dirtied the view on open). The
structurizr web application runs automatic layout **on display** exactly
when a view has no stored layout — the `autolayout` statement exists to
parameterize that pass — and renders stored layout verbatim otherwise.

| Item | Deliverable | Exit gate |
|---|---|---|
| **M6-1** | headless seam — `layout::view_needs_layout` (no non-zero coord on the RENDERED view, filtered→base like `scene::build`), `display_params` (view `automaticLayout` else the reference LR defaults — dialog params never used for display), `auto_display` (bundled Layered only, never `dot`), `seed_overlay` (save-side twin), `scene::Overrides::display` (user overlay merges OVER the engine per id); shell: the display pass runs in `rebuild_scene` UNDER the overlay (session display, NEVER dirty) and `save_workspace` seeds untouched ids so the JSON persists what was displayed; `autolayout_once` (dirty-on-open) removed | 4 `layout.rs` units + `crates/oxidizr-viewer/tests/auto_display.rs` on big-bank-plc; quartet + `ci gui` + full tier 8/8 + bless 307×unchanged; clippy × 4 feature combos — **DONE 2026-11-20** — `a1e7dfc` |

No V-item needed: no bytes contract touched (the display pass is
session state, like the M2-5 `Overrides` stance; `oxidizr-json` output
stays file-driven — bless 307×unchanged held).

## §P3-M7 — post-v0.3.0 follow-up: view list grouped by kind (2026-11-20)

Second half of the same user report: the sidebar view list is a flat
column, so a workspace with a dozen views across five kinds is hard to
scann — "the views should be graphically split into what type of view it
is". The structurizr web UI's selector is likewise per-type; our list
now is too.

| Item | Deliverable | Exit gate |
|---|---|---|
| **M7-1** | headless seam `overview::section` (kind → `(rank, header)`, C4 granularity first: landscape → context → container → component → deployment → dynamic → filtered → image → custom) + `overview::grouped` (stable `(rank, order, key)` sort partitioned into `(header, members)` runs); shell renders `── HEADING ──` separator blocks, row rendering split into `view_row` for the line budget; `view_list`'s own sort gone (subsumed) | `overview.rs` unit over all nine kinds (fixture `NineKinds`, scrambled input, section + intra-section ranking asserted); quartet + `ci gui` + full tier + bless; clippy × 4 combos — **DONE 2026-11-20** |

UI presentation only — no bytes contract, no V-item.

## §P3-M8 — post-v0.3.0 follow-up: `oxidizr view` subcommand (2026-11-20)

User asked why the viewer is a separate binary, not an `oxidizr`
subcommand. §P3-D1 had parked a launcher as "💡 idea if demanded";
demanded, and signed off via the questions tool as the **feature-gated
in-binary** form (over a thin launcher / two binaries). Recorded as F10 in
`ADDITIONAL_FEATURES.md`; P3-D1 amended above. Goal: `oxidizr view [FILE]
[--theme …]` opens the SAME viewer window in-process when the binary is
built `--features gui`, while a default `cargo build`/`cargo test
--workspace` keeps ZERO eframe/GPU surface (R-2 holds).

The blocker: the whole eframe shell lives in `oxidizr-viewer`'s **binary**
(`src/main.rs`), which the R-2 guard treats as the one allowed GUI site
and which `oxidizr-cli` cannot call into. The shared seam must move into
the lib, `gui`-gated, so BOTH entries ride one `run`.

| Item | Deliverable | Exit gate |
|---|---|---|
| **M8-0** | decision + this decomposition (ADDITIONAL_FEATURES F10, P3-D1 amendment) | **DONE 2026-11-20** — `981a0b3` |
| **M8-1** | viewer shell `main.rs` → `#[cfg(feature="gui")] mod shell` (lib `shell.rs`) exposing `pub fn run(file, theme) -> eframe::Result` + `pub enum ThemeArg`; `main.rs` becomes `Args::parse()` → `shell::run`; R-2 guard skips `shell.rs` alongside `main.rs`, count `11` (lib.rs + 10 core modules) UNCHANGED — the shell module is not a core module | default `cargo test -p oxidizr-viewer` still builds NO eframe; `--features gui` compiles; quartet + `ci gui` — **DONE 2026-11-20** — `55a0219` |
| **M8-2** | `oxidizr-cli`: optional `oxidizr-viewer` dep + `gui = ["dep:oxidizr-viewer","oxidizr-viewer/gui"]`; `Command::View{file,theme}` (`ViewTheme` ValueEnum) dispatched to `shell::run` under `#[cfg(feature="gui")]`, else a clear "built without the `gui` feature" exit; `oxidizr-viewer` promoted to a workspace dep (already is) | default cli clippy clean (no eframe); `--features gui` clippy clean; existing CLI exit-code suite green — **DONE 2026-11-20** — `6bd7073` (+2 view tests, 24 CLI tests; live smoke: `oxidizr view big-bank-plc.dsl` opens) |
| **M8-3** | `cargo xtask ci gui` leg ALSO clippies `-p oxidizr-cli --features gui` (so the in-binary GUI is gate-covered); README (root + viewer) + WORKFLOW command table note the new entry; `.github/workflows/ci.yml` mirrors both legs (gui clippy + the 3-OS `cargo check`) | `ci gui` green with both clippy invocations — **DONE 2026-11-20** |

Purely additive — `view` never enters the `oxidizr-api` pipeline, so the
bless 307×unchanged tripwire and all byte-parity paths are untouched.
Standalone `oxidizr-viewer` binary is RETAINED (the GUI's default entry;
the flake runs it). No V-item: no bytes contract touched.

## §P3-M9 — post-v0.3.0 follow-up: Windows CI reload failures (2026-11-20)

The windows-latest CI leg failed the four event-driven reload tests
(`wait_for_edge` 5 s timeouts); the silence-only sibling test passed —
the signature of "no event ever sets `pending`", not timing. The
reference decides: notify 8.2.1 `src/windows.rs` maps EVERY
`FILE_ACTION_MODIFIED` (the only action `ReadDirectoryChangesW` reports
for a content write OR an mtime touch) to `EventKind::Modify(ModifyKind::Any)` —
it NEVER emits `Modify(Data)` the way inotify/fsevent do. `interesting()`
accepted only `Modify(Data|Name) | Create | Remove`, so plain `fs::write`
events were dropped pre-filter on Windows. (The save tests rode the
rename `Modify(Name(To))`/`Create` classes and survived; the same
signature fits the CI report exactly.)

| Item | Deliverable | Exit gate |
|---|---|---|
| **M9-1** | `interesting()` accepts `ModifyKind::Any` under `#[cfg(target_os = "windows")]` ONLY (justifying comment; `#[cfg]` is illegal inside `matches!` patterns → two cfg-gated arms) — safe because the P3-D6 edge contract is the path filter + settle + content-hash diff in `poll()`: a metadata-only `Modify(Any)` still hash-matches and stays silent; Linux/macOS keep the D6 classes verbatim. Unit test `interesting_maps_the_per_backend_event_classes` pins the platform mapping (Category A, all OSes) | Linux quartet + `ci gui` + full tier + bless; **negative control on the wine windows-gnu leg: pre-fix code reproduces the exact 4 CI failures, post-fix 6/6 reload tests + full 94-lib viewer suite green**; windows-latest CI re-run is the authoritative NTFS check (wine synthesizes RDCW from inotify — caveat noted) — **DONE 2026-11-20** |

No bytes contract, no V-item (viewer-internal watch filter); D6 text
stands — the hash compare was always the authority. Notes:
`pitfalls/notify-windows-modify-any.md`, wine-leg "Last useful" stamped.

## §P3-M10 — post-v0.3.0 follow-up: roomier display layout + right-drag pan (2026-11-20)

Two UX reports on the shipped viewer: (a) "the automatic layout looks
weird, it doesn't leave much space"; (b) "mouse controls for moving are
missing". Both are presentation-only, session-state surfaces (the display
geometry is §P3-M6 session state until a save seeds it) — no bytes
contract, no V-item.

(a) Cause: a DSL `autoLayout` statement stores `AutomaticLayout` at the
reference defaults **rank 100 / node 50**; with the 350 px-wide default
boxes that packs ranks to a 450 px pitch and stacks same-rank boxes edge
to edge (measured min box gap 50 px) — cramped next to the web app's
generous Graphviz output. Fix: `display_params` floors the separations at
the new `DISPLAY_FALLBACK` (LR, rank **200** / node **120**, edge 50,
vertices) with graphviz LOWER-bound semantics — a file asking for more
keeps its value, the direction and vertex routing always stay the file's;
no stored layout → the full fallback. The floor applies to the DISPLAY
pass only: the dialog path and `AutomaticLayout::default` (a JSON default,
reference-verified) are untouched, so an explicit dialog run is still
precise file-for-file and no output byte changes.

(b) Cause: pan rode middle-button/Space only — undiscoverable on a
two-button mouse; right-drag (the draw.io/Visio convention) was unused.
Fix: `PointerButton::Secondary` joins Middle/Space as a pan trigger in
the §P3-M3 press state machine; `?` hover text + viewer README updated.

| Item | Deliverable | Exit gate |
|---|---|---|
| **M10-1** | `DISPLAY_FALLBACK` + separation floor in `display_params`; two exact-placement display tests re-pinned (pitch 450 → 550) + `display_params_floor_the_separations_only_upwards` | quartet + `ci gui` + full tier + bless; clippy × 4; **DONE 2026-11-20** |
| **M10-2** | right-button pan in `shell.rs` (press chain + `Drag`/`?` docs) + README nav bullet | same + live window smoke — **DONE 2026-11-20** |

## §P3-M11 — post-v0.3.0 follow-up: input/rendering UX batch (decomposition, 2026-11-20)

Eleven live-use reports on the shipped viewer, all presentation-only
session-state surfaces (no bytes contract, no V-item — the JSON stays
the writer's output regardless of anchors, z-order or gesture handling;
M11-6 changes only what OUR display engine produces for the DISPLAY
pass, and dialog runs were always engine-defined). More than four
modules (shell, paint, scene, commands, oxidizr-layout), so the charter
requires this decomposition before implementation. Commit grouping in
the `C` column — each C lands on green gates.

| Item | Deliverable | C |
|---|---|---|
| **M11-1** | Swap canvas button roles: left-drag on empty canvas PANS, right-drag LASSOS (press chain + `press_mode` empty branch + `Drag` doc). Click/select/move/waypoint stay left; Middle/Space keep panning — **DONE 2026-11-20** | C1 |
| **M11-4** | Trackpad: pinch zooms at the pointer (egui folds ctrl-scroll/magnify into `zoom_delta()`), two-finger scroll PANS, wheel keeps zooming (per-event `MouseWheelUnit`: `Line`/`Page`=wheel→zoom, `Point`=trackpad→pan) — **DONE 2026-11-20** | C1 |
| **M11-10** | Hover tooltip restored: hand-rolled `Area::new(Id::unique(..))` block (fresh id every frame — the flicker) becomes egui's `response.on_hover_ui_at_pointer` (stable identity, 12 px gap, same `tooltip_text`) — **DONE 2026-11-20** | C1 |
| **M11-2** | Remove arrow handles: **Delete/Backspace** clears ALL waypoints of the selected edge; handles now also PAINT on edge hover and `waypoint_hit` accepts the hovered edge, so Alt+click deletes without selecting first | C2 |
| **M11-8** | Moving boxes carries their arrowheads: dragging N boxes translates the interior waypoints of edges whose BOTH ends move; **Ctrl+drag detaches** (waypoints stay). Preview==commit via one `Command::Group` | C2 |
| **M11-3** | Arrows land on box SIDES, not centres: `scene::build` replaces centre endpoints with the centre→next-point ray clipped to the rect border (same dx/dy rule as the engine's `endpoint(None)`, so engine-routed sides agree) | C3 |
| **M11-5** | Text never covered: `paint_scene` z-order split — all fills/strokes, then edges, then ALL galleys (nested boxes no longer paint over a parent's label) | C4 |
| **M11-7** | Search result rows show the element's icon (`icon_cell`, `▪` fallback) | C5 |
| **M11-9** | Opened-file path appears ONLY in the toolbar edit: window Title and status bar drop it (status keeps the view key + dirty state) | C5 |
| **M11-11** | Click object → right panel shows an **Info** block (kind/id/name/metadata/description/group/tags/URL; edges: source → destination + labels) above the Key | C6 |
| **M11-6** | Engine routes stop stacking: port spread in `layered.rs` — edges sharing a (node, side) distribute their anchor along the side by far-end cross order (N edges → N+1 slots), replacing the shared side-midpoint anchor | C7 |
| **M11-12** | Oxidizr-only sidecar: non-contract per-file data persists in `<stem>.oxidizr.json` (versioned envelope, atomic write, our own JSON — NEVER fed to or from the contract writer, NEVER adopted as an open/reload source); first user = per-view viewport memory (zoom + centre restored on reopen). User directive 2026-11-20, registered in ADDITIONAL_FEATURES.md | C8 |

Decisions: button swap = pan↔lasso only (draw.io keeps select/edit on
left — click, box-move, waypoint drag, URL-click all stay Primary; a
right-press with zero motion clears the selection, mirroring the old
left-lasso behavior); wheel-vs-trackpad is distinguished by scroll
UNIT (winit maps wheel→Line, touchpad→Pixel; no `MouseWheelSource` on
Linux — unit is the portable signal); the `zoom_modifier` fold in egui
means ctrl-scroll events must be SKIPPED in our loop to avoid
double-zoom; waypoint carry moves only fully-contained edges (an edge
leaving the moved group keeps its stored waypoints — rerouting them
would fight the user's stored geometry); M11-3 recomputes anchors in
SCENE, not per engine, so hand-placed views without vertices benefit
too; M11-6 anchor spread is engine-side because only the engine knows
the rank geometry.

## P3-D10 Verification list (empirical, before dependent code)

- P3-V1 auto-layout parity intent: what the reference web tool/dot produces for
  representative C4 views (rank direction per view type, boundary nesting) — drives
  `Layered` tuning; result is our {x,y}, contract-safe by definition.
  **CLOSED 2026-11-20** (monorepo `ad34036`): the web tool lays out
  **client-side** via JointJS DirectedGraph = **dagre 1.1.8**
  (`structurizr-diagram.js:6844` `applyAutomaticLayout` →
  `joint.layout.DirectedGraph.layout(graph, {rankSep, nodeSep, edgeSep,
  setLinkVertices, setLabels, rankDir, marginX/marginY 0, resizeClusters,
  clusterPadding 50})`); `dot` exists only as the server-side
  `GraphvizAutomaticLayout` (`dot -Tsvg` + SVGReader read-back, margin 400 —
  the M4 `dot-backend` reference). **No per-view-type rank direction**: every
  view type uses the dialog choice, default **LeftRight**
  (`structurizr-ui.js:8-12`); a stored `configuration.automaticLayout`
  re-runs the layout EVERY render with stored params and locks the diagram
  non-editable (`structurizr-diagram.js:399` — M3 decision open: oxidizr may
  stay editable). Reference defaults (`AutomaticLayout.java:8-12`) =
  LR / rankSep 100 / nodeSep 50 / edgeSep 50 / vertices true — already
  mirrored in `oxidizr-core::AutomaticLayout::default()` (verified 1:1).
  dagre's phase list (greedy DFS cycle removal → ranking (network-simplex
  default) → merge ordering + priority coords) **is** the §P3-D5 MVP step
  list; our longest-path cap + fixed barycenter sweeps are the SANCTIONED
  MVP simplification (quality bar "usable C4", Graphviz parity P4-D13,
  P3-D0 visual parity out of contract). Boundaries lay out as dagre
  CLUSTERS (resize to children + 50 px padding) — reproduced naively by
  the scene's member-union boundary rects; **pad deviation recorded on
  the §P3-M2 M2-5 row: ours stays `BOUNDARY_PADDING = 20` (the M1 chrome
  constant), not dagre's internal 50.**
- P3-V2 reference font/theme loading (offline story) → confirm bundled-substitution
  policy in P3-D2. **CLOSED 2026-11-19** (monorepo `ad34036`): the reference **never
  loads web fonts** — `fontUrl` has ZERO consumers (JS/Java/CSS/JSPF); everything
  renders in the system stack `structurizr.ui.DEFAULT_FONT_NAME` =
  `'Tahoma, Verdana, Helvetica, Arial'` (`structurizr-ui.js:1`), and
  `views.configuration.branding.font` (JSON schema 1309) is stored but never read.
  Themes: built-in names resolve to the app-BUNDLED `/static/themes/<name>/theme.json`
  (`structurizr-ui.js` `loadThemes` — offline by construction); only `http…` entries
  are fetched, and a failed fetch pushes an EMPTY theme stub (graceful,
  diagram renders with base styles). → P3-D2 bundled-substitution policy CONFIRMED
  (parity-plus: we never need the network).
- P3-V3 theme icon corpus scan: SVG-with-text frequency; confirm atlas approach.
  **CLOSED 2026-11-19**: the bundled theme corpus (`structurizr-themes/`) holds
  16 themes / 5509 icon files, **100% PNG** (5485 theme.json icon refs, all
  relative `.png`; ZERO SVG files, hence ZERO SVG-with-text). → raster texture-atlas
  approach CONFIRMED; usvg/tiny-skia only ever rasterizes foreign/custom-theme
  icons; no text-in-SVG special case.
- P3-V4 `automaticLayout` fields honored by reference consumers when geometry exists
  (do we clear or keep the block on manual edit? match Lite behavior).
  **CLOSED 2026-11-20** (monorepo `ad34036`): the block is **permanent** —
  consumers READ it, nobody writes it: `setAutomaticLayout` has ZERO call
  sites in main sources (only the `ModelView.java:128` setter definition).
  When a view carries the block the web tool (1) re-runs the client layout
  on EVERY render with the stored params, overriding stored geometry
  (`structurizr-diagram.js:989-996`), and (2) locks the diagram
  NON-EDITABLE (`:355` canvas-drag-only, `:400` `editable = false`) — so
  "manual edit clears the block" never happens: manual editing is simply
  impossible while the block exists. `applyAutomaticLayout` (`:6844`)
  calls `removeAllVertices()` before EVERY layout run — the
  `vertices`/`setLinkVertices` flag controls whether the engine
  REGENERATES waypoints, not whether existing ones survive (the P3-D4
  "overwrite vs keep" reading is settled: always overwrite).
  Server side: `GraphvizAutomaticLayout.apply(Workspace)`
  (`structurizr-autolayout …:200-230`) honours the block per view on every
  apply — Lite behavior = honour-always, clear-never. **Decision (P3-D4
  editor, ratified 2026-11-20):** oxidizr keeps the diagram EDITABLE with
  a stored block (the V1 row's open decision — the reference lock is web
  UX; feature parity per P3-D0 makes editing our headline); the block is
  permanent too — manual edits live in the overlay (session, overrides
  win), save writes GEOMETRY ONLY (`x/y` + vertices), the block
  round-trips untouched. Dialog parity fact logged: the reference dialog
  footer (`auto-layout.jspf`) houses **Bring-elements-back** (magnet) and
  **Remove-all-vertices** buttons next to Auto-layout — our M3 dialog
  gains both.
- P3-V5 filtered-view evaluation semantics corner cases (relationship filters,
  `directRelationshipsOnly`) vs `core::filter::eval`. **CLOSED 2026-11-19** against
  monorepo `ad34036`: the reference evaluates client-side ONLY (Java `FilteredView`/
  `ViewSet.newFilteredView` just stores mode+tags; `structurizr-export` has zero
  `FilterMode` consumers). Webapp evaluator (`structurizr-diagram.js`
  `includeOnDiagram`/`createArrow`, `structurizr-workspace.js` `getAllTagsFor*`):
  **Include** = keep iff ANY filter tag (each trimmed) hits the effective stack —
  empty tag list keeps NOTHING; **Exclude** (and any absent/non-Include mode —
  the reference tests `mode === "Include"` and `else`s everything else) = drop on
  ANY hit, empty keeps everything. Effective stacks: element = kind defaults +
  stored, deployment instances ALSO match the linked software-system/container
  stack; relationship = its defaults+stored, any `linkedRelationshipId`-chain hop
  can supply the hit. Keep-rule: a relationship survives only when it passes its
  own filter AND both endpoint elements survive (arrows are gated on the rendered
  boxes). Filter tags trim; item tags match verbatim. `directRelationshipsOnly`:
  ZERO hits in the whole monorepo — no such reference option, N/A. (The
  `filter.jspf` tag toolbar is the session-side UI feeding the SAME evaluator —
  its persistence question is P3-V7.) Cloned in `oxidizr-core::filter`.
- P3-V6 `notify` event coalescing behavior on each OS (debounce tuning), Windows
  rename semantics on save. **Linux leg CLOSED 2026-11-20** — probe facts + the
  filter/debounce/hash posture in §P3-M4 above; Windows/macOS legs stay a passive
  CI observation (the posture is OS-agnostic, nothing depends on their exact shape).
- P3-V7 reference tag-filter persistence: does the web UI filter write the view's
  stored `filter` object (workspace JSON) or stay session-only? → decides our default
  (P3-D3a); check `filter.jspf` + `structurizr-diagram.js` filter wiring + whether a
  filtered+saved workspace JSON changes. **CLOSED 2026-11-19** (monorepo `ad34036`):
  **SESSION-ONLY**. The toolbar (`filter.jspf`) mutates the diagram module's session
  object (`structurizr-diagram.js:89` `var filter = { id, active, tags }`) through
  `setFilter`/`filterOff`, which only bump `filter.id` and re-render (`runFilter`) —
  no write to any view's stored `filter`, no localStorage/sessionStorage anywhere in
  the wiring. Seeding: `?tags=` URL param or `structurizr.filter.tags` config property
  (which also picks the chip list via `getUserDefinedTags()` = all tags minus the
  kind defaults). Active filter = ANY selected tag hits the effective stack (same
  include-side semantics `includeOnDiagram` uses — already cloned in
  `core::filter::eval`); inactive = keep everything. → Our P3-D3a default is pinned:
  session-only, model file never touched.
- P3-V8 reference PNG/SVG export anatomy: what "diagram and key" composition means
  (key appended? margins?) — informational for our export design; NO pixel parity
  implied (P3-D0).

## Risks

- R-1 layout quality disappoints on big nested graphs (mitigation: MVP scope shipped
  first with the P4-D13 upgrade path pre-designed; editing is the escape hatch by
  design; `dot-backend` optional; shallow-graph focus).
- R-2 GUI logic leaking into the eframe shell, killing headless tests (mitigation:
  P3-D1 module boundary enforced by review + `oxidizr-viewer` lib/bin split lint).
- R-3 reload-remap mis-assigning geometry (mitigation: shared core::reconcile,
  ambiguous = conservative drop + report, property tests).
- R-4 wgpu/eframe API churn (mitigation: version pin, thin shell, upgrade as a
  planned chore).
- R-5 SVG edge cases (mitigation: warn-and-placeholder fallback path from day one).
