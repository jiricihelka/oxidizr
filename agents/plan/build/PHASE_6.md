# PHASE_6.md — detailed plan: GUI improvements — dependency-arrow editing + help-text rendering

**Status: PLANNED 2026-11-28** — scope ratified by the user via the questions
tool (2026-11-28); implementation not started. Position: after P5R / `v0.5.0`;
first feature phase since P4 on the refactored tree. No tag at close — the
release ritual rides a later tail unless the user asks for one.

User charter (verbatim, 2026-11-28): *"Focus on editing of dependency arrows
and rendering of help texts. This can diverge from structurizr, if it makes UX
better. Explore what can be improved and propose the changes now. After
proposal, for accepted changes ask questions now. After planning phase, you
will need to be able to work autonomously."*

Ratification answers (questions tool, 2026-11-28 — the decision anchors):

- Scope: **all four groups accepted** — A (route UX), B+C (model editing +
  arrow ops), D1+D2 (help overlay + details panel), D3+D4 (markdown + docs).
- Undo: **single session history** (one Ctrl+Z stack for geometry + model).
- Create gesture: **port handles + drag** (element→element).
- Keys: **Del deletes the edge**; waypoint clearing moves to **Ctrl+Shift+K**.
- Markdown: **egui_commonmark** allowed (gui-only dependency).
- DSL sources: **model edits BLOCKED** — arrow create/delete/reconnect/label
  are `.json`-source workspaces only (no implicit sidecar authoring).

## P6-D0 Charter and freeze scope

**Frozen (unchanged from the standing contract)**:
- JSON byte contract for every path that exists today: parse, validate,
  export, geometry save. `cargo xtask e2e bless` stays a NO-OP at every
  commit; goldens never re-blessed for P6 reasons.
- The viewer's geometry pipeline (scene build, paint, `LayoutOverlay`,
  hit-testing math) keeps working for workspaces that never touch an arrow op.
- Headless dependency graph: `oxidizr-viewer` WITHOUT `gui` gains ZERO new
  deps (egui_commonmark rides the `gui` feature only — `cargo tree -e normal`
  check at M0 and at close).
- No `unsafe`, std-only in our code (GUI-stack deps exempt as before);
  errors as values; the quartet + `cargo xtask ci gui` stay the per-commit
  gate on viewer work.

**Open for P6 (with rationale recorded below)**:
- The P3-D4 editor rule is AMENDED (P6-D2): model content becomes editable
  through an explicit command layer, only for `.json` sources, only on
  explicit user gestures.
- Undo granularity moves from per-view stacks to ONE session history (D17 in
  `DEVIATIONS.md` — the official layout editor undoes per diagram).

**Out of scope**: element create/delete/move-between-containers; relationship
`url` editing; ADR/decision rendering (docs window shows `!docs` sections
only); editing through a filtered-view mutation of the FILTER itself;
anything DSL-source model editing. Everything else Structurizr-lacks stays
parked in `ADDITIONAL_FEATURES.md`.

## P6-D2 Editor rule v2: model editing through invertible commands

P3-D4 kept the workspace immutable and every command inside
`LayoutOverlay`. P6 amends, not abolishes:

- Geometry commands NEVER touch the model (unchanged); model commands NEVER
  write the geometry overlays (single exception: relationship DELETE removes
  the deleted id's stale overlay entries, snapshotted for undo).
- New `commands::ModelCommand` enum, same invertible from/to discipline as
  `Command` (no closures): `CreateRelationship`, `DeleteRelationship`,
  `ReconnectRelationship { id, role: Source|Destination, from: Id, to: Id }`,
  `SetRelationshipText { id, field: Description|Technology|ViewLabel, from,
  to }` (`ViewLabel` = the CURRENT view's `RelationshipView.description`
  override).
- Apply mechanism: `Viewer.workspace` stays `Option<Arc<Workspace>>`; a model
  command runs `Arc::make_mut` (COW clone — one clone per user gesture, fine
  at 10k-element scale, re-measure at close only if flagged) then
  `rebuild_scene()`; the inverse re-runs through the same door.
- New relationship payload: fresh id from a viewer-local helper that rebuilds
  `oxidizr_core::ids::IdGenerator` by `found()`-ing every element+relationship
  id then `generate()` — the JSON reader's own registration behavior, so ids
  match what the official tooling would allocate. Initial field values mirror
  the reference defaults: description `"Relationship"` (the DSL default),
  technology/url/properties empty, stored tags `["Relationship"]` — VERIFY at
  M3 against the pinned monorepo `Relationship` constructor (V2).

## P6-D3 Single session history

`Viewer.histories: HashMap<view key, History>` becomes ONE `history`. Steps
become `Edit { Geometry { view_key, Command }, Model(ModelCommand) }`: undo
of a geometry step re-applies to ITS view's overlay (marking that view dirty
even if it is not the current one) and repaints; model steps re-apply to the
workspace. View switching no longer resets the stack; opening/closing a
workspace does. Cap 200 unchanged. Rationale: model edits are global —
per-view stacks would let a Ctrl+Z in view A desync the model against view B
(the question the user answered "single session history"). UX divergence vs
the official layout editor's per-diagram undo: registered as **D17**.

## P6-D4 Source gate: model editing is `.json`-only

`model_editing_allowed(display)`: true iff the display path has a `json`
extension (case-insensitive), `display != "-"`, and `companion_dsl(display)`
is `None`. Consequences: a `.dsl` source is blocked before any save; after a
geometry save the sidecar-preferred reopen STILL shows a DSL sibling ⇒ stays
blocked (a DSL-sourced workspace never gains model editing by round-tripping
through our own sidecar — the user's ruling's whole point); stdin blocked.
Blocked affordances do not appear (no port dots; Del on an edge keeps the old
strip-waypoints meaning is NOT kept — see D5: Del becomes delete-elsewhere,
and on a blocked source Del on an edge shows the status note
`model editing needs a plain .json source — the DSL file is never rewritten`).

## P6-D5 Keys and gestures (the final binding table)

The single source of truth is the new headless `shortcuts.rs` table (P6-D7);
this is its content:

| Gesture / key | Result |
| --- | --- |
| hover element (model-editing source) | port dots at the 4 side midpoints |
| port press → drag → release on another element | CREATE relationship (Esc or empty release cancels; new edge lands selected) |
| press endpoint handle (selected edge) → drag to element | RECONNECT that endpoint |
| Del / Backspace on a selected edge | DELETE the relationship (undoable) |
| Ctrl+Shift+K on a selected edge | strip all waypoints (was: Del — swapped in M4) |
| selected edge, segment midpoint dots | press-drag inserts a waypoint and drags it (commit = one command) |
| element selected, arrows | nudge by `GRID_PX`; Shift = 10× |
| grabbed waypoint (last-touched handle), arrows | nudge that waypoint |
| F1 or `?` | help overlay (Esc closes it BEFORE clearing selection) |
| docs button in the details panel (or F2) | documentation window |
| unchanged | click-select, dblclick insert, Alt+press delete vertex, waypoint drag, lasso, pan, Ctrl+Z/Y, Ctrl+S, double-click fit |

Escape priority: help open → close help; else selection cleared (existing).

## P6-D6 RelationshipView injection (what makes new edges render)

On CREATE, inject a `RelationshipView` (fresh geometry: no vertices, no x/y —
router computes) into every **static** view (landscape, context, container,
component) whose included-element set contains BOTH endpoints — the official
loader's add-relationship behavior. Skipped: dynamic/deployment (explicit
step/relationship sets), filtered (geometry derives from base), image.
On DELETE, remove the model relationship + its `RelationshipView` from ALL
views (snapshot for undo) + stale overlay entries from ALL session overlays.
Reconnect and label edits touch the model row (+ the view's RelationshipView
only for `ViewLabel`). Undo restores exactly the snapshot, not a re-derive.

## P6-D7 Help texts: one table, three renderers

- New headless lib module `shortcuts.rs`: `fn reference() -> Vec<Section>`
  (title + entries `{ keys, action }`) covering the WHOLE binding table
  (existing gestures included). Unit tests: non-empty sections, unique
  actions, keys strings non-empty. The shell help overlay (F1/`?`) and the
  empty-canvas hint render from it — the table is the only place bindings
  are written down (V6 audits table vs canvas handlers).
- Details panel (right column, above the legend): selected element →
  kind/name/description (markdown-rendered)/metadata/group/stored tags/
  properties/URL link; selected edge → source→target names, description,
  technology, id, per-field editable when model-editing is allowed (C4):
  description, technology, view label override. Editing commits ONE
  `SetRelationshipText` on Enter/focus-loss (empty = None).
- Documentation window (F2 / docs button): `!docs` `Section`s attached to the
  selected element + workspace-level sections, `order`-sorted; `Format::Markdown`
  → egui_commonmark; `Format::AsciiDoc`/absent → monospaced verbatim with a
  note (no AsciiDoc renderer). Decisions/ADRs: out of scope.
- egui_commonmark `0.26` (egui ^0.37 — verified crates.io deps 2026-11-28),
  `default-features = false`, minimal features (no image loading — icons
  already have their own pipeline); `Markdown` state cache lives on `Viewer`,
  content-keyed, size-capped; gui-feature only.

## P6-D8 Dirty + save rules

`model_dirty: bool` beside the geometry dirty set. Save writes
`workspace.clone()` + per-view overlay seeding exactly as today (model edits
already live in the owned workspace, so the shared writer covers them — no
second serializer). Save fires when `model_dirty` OR geometry keys are dirty;
the FILTERED-view block stays ONLY for geometry-only saves (model edits are
view-independent, so a model-only save from a filtered view is legal).
`model_dirty` clears with the geometry set on a successful write; the status
bar chip reads `model edited` next to the per-view dirty dots.

## Verification list (P6-V)

- V1 every `ModelCommand` round-trips: apply → undo is identity (unit,
  per variant, including the overlay-snapshot and view-injection legs).
- V2 reference-shape witness: a created relationship's JSON (id allocation,
  `tags: ["Relationship"]`, description default) matches what the reference
  produces — Category B at close: oracle reads + re-exports an edited fixture
  (`oracle verify` sidecar legs extended with one P6 fixture) + the pinned
  `Relationship.java` constructor re-read at M3.
- V3 `cargo xtask e2e bless` NO-OP every commit (no model-edit code on any
  parity path).
- V4 `cargo tree -e normal -p oxidizr-viewer` (no features) unchanged from
  the P5R tree — M0 and close.
- V5 `cargo xtask ci gui fmt clippy test refs` green on EVERY commit
  touching the shell or the gui feature.
- V6 shortcuts table == shipped bindings: manual audit checklist at M6
  against `canvas.rs`/`mod.rs` handlers, recorded in the wave row.
- V7 gate matrix: model editing allowed for plain `.json` only; blocked for
  `.dsl`, sidecar-preferred reopen (DSL sibling exists), stdin — headless
  unit tests on the gate function.
- V8 untouched workspaces save byte-identically: the existing headless save
  tests plus bless prove geometry-only saves are P5R-identical.
- V9 close: quartet + full tier + doc-zero + `ci windows` + bless +
  Category B (oracle verify + export-verify, `TZ=UTC`).

## Waves (each: quartet + `ci gui` green = commit; RESUME updated)

| Wave | Scope | Notes |
| --- | --- | --- |
| **P6-M0** | egui_commonmark spike behind `gui` (0.26, minimal features); headless `shortcuts.rs` table drafted with ALL bindings | V4 measured in this commit; CHANGELOG `[Unreleased]` lines start — **DONE 2026-11-28**: `egui_commonmark 0.26` pinned workspace-side `default-features = false` (drops load-images/regex; `pulldown-cmark` is a hard dep of the crate), rides `gui = [... dep:egui_commonmark]`; rust_version 1.95 matches the GUI stack already (epaint 0.37 = 1.95), MSRV 1.88 unchanged headless; V4: `cargo tree -e normal -p oxidizr-viewer` = zero egui/commonmark; `shortcuts.rs` = const `SECTIONS` data + `all()` (3 unit tests: populated, actions unique, key labels unique); lib-core boundary guard re-counted (15 files); hygiene lesson: bare `F1`/`F2` + the phrase "the reference" are prose-pattern violations in rustdoc/comments (string literals exempt for F-keys) — pitfall `refs-hygiene-prose-patterns.md`; gates: `cargo xtask ci gui fmt clippy test refs` all green (gui + clippy fixed en route: `too_many_lines` → const data, `needless_borrow`) |
| **P6-M1** | single session history (`Edit` step enum, shell wiring, tests) | behavior change noted in help + CHANGELOG; D17 entry lands with this wave — **DONE 2026-11-28**: `Edit::Geometry { view_key, command }` + session `History { run_geometry, undo→Option<view_key>, redo, reset }` (cap 200 kept); shell: one `Viewer.history`, `edit_current_view` runs through it, `step_history` marks the STEP's view dirty; `adopt()` resets (opens), view switches don't; commands.rs header re-worded (per-diagram stacks named as the deliberate divergence); tests: history test re-targeted + NEW `history_walks_through_views` (LIFO across A/B, forward redo, reset); CHANGELOG Changed line; `ci gui fmt clippy test refs` all green (110 viewer tests) |
| **P6-M2** | route UX: midpoint insert dots, endpoint-handle rendering (visual only yet), arrow-key nudge (element + grabbed waypoint), Ctrl+Shift+K added as alias | headless hit-tests for dots; Del unchanged until M4 — **DONE 2026-11-28**: `hit::nearest_midpoint` (nearest dot, tolerance, degenerate lines — 1 unit test); `paint::paint_route_kit` — SELECTED edge paints white insert dots (accent rim, `INSERT_DOT_PX`) per segment + combined-fill-stroke endpoint squares (`ENDPOINT_HANDLE_PX`, painted ahead of the reconnect wave), hover excluded — 2 paint tests (counts by fill/stroke identity, hover-zero); `commands::insert_waypoint` factory = ONE `SetWaypoints` splicing the snapped target at the segment index (undo removes exactly the insert — round-trip test); shell: `Drag::Insert` (press chain after `waypoint_hit`, preview==commit via shared `drag_target`, `insert_dot_hit` on the SELECTED edge only), `Viewer.grabbed` set by waypoint/insert commits and dropped by Esc/adopt/non-drag route edits (`drop_grab`), `nudge_selection` = grabbed waypoint else selected elements (grid step, Shift 10×, one command per press), Ctrl+Shift+K alias added beside Del; `ci gui fmt clippy test refs` ALL GREEN + bless NO-OP (113 viewer lib tests) |
| **P6-M3** | headless model core: `ModelCommand` set + apply/undo on `Arc<Workspace>`, id helper, injection/removal rules, unit tests | reference re-read for the create-payload (V2 first half); quartet (headless) |
| **P6-M4** | canvas model ops: gate helper, port-drag CREATE, endpoint RECONNECT, Del=delete + Ctrl+Shift+K exclusive, `model_dirty` + save rules | the biggest shell diff; preview==commit invariant held; manual smoke list in the wave row |
| **P6-M5** | details/inspector panel with markdown descriptions + editable edge fields | first real egui_commonmark use; `Markdown` cache on `Viewer` |
| **P6-M6** | help overlay (F1/`?`) from `shortcuts.rs`, Esc priority, tooltips for port dots/handles/dots, hint chip | V6 audit here |
| **P6-M7** | documentation window (`!docs` sections, markdown + verbatim fallback) | per-element + workspace-level |
| **P6-M8** | close: CHANGELOG review, viewer README + `docs/` refresh, PHASE_6/PROGRESS/RESUME journals, full V-list | V9 chain; no tag |

## Risks

- **R-1** egui_commonmark API friction (state-cache semantics, panel sizing)
  — contained by the M0 spike; worst case M5/M7 degrade to wrapped plain
  text with a registered follow-up idea, phases keep shipping.
- **R-2** `Arc::make_mut` clone cost on huge workspaces — one clone per
  explicit gesture, acceptable; measured once at close, not per commit.
- **R-3** history unification regressions (undo across view switches) —
  pinned by new unit tests in M1, pre-existing command tests re-targeted.
- **R-4** view-injection semantics on exotic view mixes — decided narrow in
  P6-D6 (static views only); dynamic/filtered/deployment untouched.
- **R-5** reload racing a model edit — model commands run on the UI thread
  like every edit; the existing reload-on-UI-thread seam holds.
