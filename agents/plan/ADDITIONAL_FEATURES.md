# ADDITIONAL_FEATURES.md — features beyond Structurizr parity

Registry of everything oxidizr offers that the reference does NOT have. Companion to
`DEVIATIONS.md` (which covers deliberate differences where reference behavior exists).

Rules:

1. Purely additive: a feature may never change what parses to what, or the bytes of
   default output, for workspaces that don't ask for the feature. Parity paths (parse,
   validate, JSON write) stay reference-exact (see DEVIATIONS.md contract).
2. Files produced by any feature must remain standard Structurizr DSL/JSON readable by
   the official tooling — features that would make oxidizr-written files unreadable by
   Structurizr are out of scope, period.
3. Statuses: ✅ agreed (built only when its milestone arrives) · 💡 idea (NOT agreed —
   needs explicit sign-off via the questions tool before any implementation work).

## Agreed features

Scheduling lives in `PLAN.md`: F1+F2 land with Phase 1, F4's harness from day one
(xtask); F3/F5/F6/F7 and the `oracle` command are Phase 4; F8 rides along with JSON
output (Phase 1 flag); F9 is the Phase 1T interlude (test infra, `build/PHASE_1T.md`).

### F1 — `reconcile`: re-attach layout ids after DSL edits ✅

- Problem: the shared sequential id counter regenerates ids on every parse, so reordering
  or inserting DSL statements shifts ids and silently detaches saved `{id,x,y}` layout
  geometry in workspace.json. Structurizr offers no mitigation beyond the
  `structurizr.dsl.identifier` property it writes.
- Feature: given edited DSL + an existing layout JSON, remap element/relationship ids via
  `structurizr.dsl.identifier` and rewrite the layout references; report ambiguous/unmatched
  entries instead of guessing.
- Compatibility: operates on plain Structurizr input, emits plain Structurizr JSON. None.

### F2 — `fmt`: canonical DSL formatter ✅

- Canonical `.dsl` formatting: indentation, quoting, statement ordering. Structurizr has
  no formatter.
- Hard invariant (golden-tested): `fmt(x)` parses to the same workspace as `x`. The
  `structurizr.dsl` portability property keeps the ORIGINAL source text exactly (the
  formatter must not substitute its own output when producing that property).
- Compatibility: output is valid DSL a human would have written; none.

### F3 — `diff`: workspace-aware diff ✅

- Workspace↔workspace (DSL and/or JSON) diff with old→new element id mapping, layout
  breakage report (which geometry will detach), and categorized change summaries: model
  elements/relationships, views, styles, properties, documentation.
- Compatibility: read-only analysis; none.

### F4 — `oracle`: golden generation/verification against the official tool ✅

- Harness commands that run the official `structurizr` application **binary**
  (`$OXIDZR_STRUCTURIZR_BIN` → PATH; pinned version; the nix dev flake provides it) to
  (re)generate golden JSON from fixtures and verify oxidizr output against them; records
  the tool version used (agents/plan/TESTING.md). The reference tool is slow: runs
  are sparing (goldens committed) and every such test sits behind the default-off
  `reference` feature.
- Compatibility: development-time only; never a runtime dependency — the default test
  suite and shipping binary remain pure Rust and Java-free.

### F5 — `lsp`: Language Server for `.dsl` ✅ (agreed 2026-11-14)

- Live editor diagnostics on save, keyword + identifier completion, hover docs, built on
  the CST; makes D1 diagnostics shine in any LSP-capable editor. Structurizr has only a
  web editor.
- Compatibility: speaks LSP, edits nothing on disk without an explicit editor action;
  none.

### F6 — `query`: machine-readable workspace inspection ✅ (agreed 2026-11-14)

- Scriptable extraction instead of grepping workspace JSON: `query elements --tag=Container
  --format=json`, relationships, views, styles… Output is a subset of the workspace's own
  JSON shape, not a new format.
- Compatibility: read-only; none.

### F7 — `init`: workspace scaffolding ✅ (agreed 2026-11-14)

- Templates lowering the C4 entry barrier: minimal, system-context, deployment,
  docs+ADRs project layouts.
- Compatibility: emits ordinary DSL/JSON layouts the official tooling reads; none.

### F8 — `--strip-volatile` flag for git-friendly output ✅ (agreed 2026-11-14)

- Drops `lastModifiedDate`/`lastModifiedUser`/`lastModifiedAgent` from written JSON so
  committed workspaces diff cleanly. Opt-in only; default stays byte-parity.
- Compatibility: output remains valid Structurizr JSON; none.

### F9 — embedded e2e API (`oxidizr-api`) ✅ (agreed 2026-11-17)

- Problem: the e2e regression suite (Phase 1T) must run hundreds of workspaces ×
  several commands in-process — launching tens of thousands of CLI processes is
  not viable, and calling library internals would break on every refactor.
- Feature: a stable, headless `Request → execute() → Outcome` seam (exit class,
  diagnostics as codes, output bytes) extracted from the CLI; the CLI binary
  becomes a thin argv/render shim over it, so tests and the shipped binary can
  never drift. Pure + total + thread-safe by contract (no env/cwd/rendering).
- Compatibility: dev-facing; the CLI lib stays `publish = false`; parity paths
  unchanged (T0 is proven byte-neutral by the existing golden/e2e suite).

### F10 — `oxidizr view`: in-binary GUI entry ✅ (signed off 2026-11-20)

- Problem: the P3 viewer shipped as a second binary (`oxidizr-viewer`); the
  user asked why the GUI is not a subcommand of the `oxidizr` entry point.
  §P3-D1 had parked exactly this as a "💡 idea if demanded" — demanded, and
  signed off (questions tool) as the feature-gated IN-BINARY form.
- Feature: `oxidizr view [FILE] [--theme light|dark|system]` opens the native
  viewer window in-process, behind a new `gui` feature of `oxidizr-cli`
  (which enables `oxidizr-viewer/gui`). Default cli builds keep a ZERO-GUI
  dependency tree; the subcommand appears in `--help` in every build and,
  without the feature, exits with a rebuild hint. The standalone
  `oxidizr-viewer` binary stays (both entries ride the same
  `oxidizr_viewer::app::run`). Decomposition: `build/PHASE_3.md` §P3-M8.
- Compatibility: purely additive — `view` never becomes an
  `oxidizr-api::Request`, so pipeline behavior and every byte-parity path are
  untouched (bless 307×unchanged remains the tripwire).

### F11 — `<stem>.oxidizr.json` sidecar for oxidizr-only data ✅ (signed off 2026-11-20, user directive)

- Problem: the viewer accumulates per-file state the Structurizr JSON
  model cannot carry (per-view viewport memory; later bookmarks, custom
  groupings…). Writing it into the workspace JSON would break the byte
  contract and put unknown keys in front of the official tooling;
  eframe's storage keys per window, not per file, and is invisible to
  the user.
- Feature: oxidizr-only data persists next to the source file as
  `<stem>.oxidizr.json` — a versioned envelope written atomically
  (temp+rename) with our own JSON (serde_json, never `oxidizr_json`)
  — and NEVER fed to or read
  from the contract writer/reader, NEVER adopted as an open or reload
  source (the layout sidecar for `big.dsl` is `big.json` — a different
  exact filename), and deleting it always loses nothing (pure cache).
  Decomposition: `build/PHASE_3.md` §P3-M11 (M11-12).
- Compatibility: none — the contract pipeline never references the
  file; workspaces that never use it see zero behavior change.

### F12 — C4-PlantUML / Mermaid diagram-model importers ✅ (signed off 2026-11-21, questions tool)

- Problem: P4-D8 promised "reference-importer scope as the ceiling" (R-1),
  but the P4-V3 inventory (pinned monorepo `ad34036`, 2026-11-21) found
  the reference has NO C4-PlantUML/Mermaid→model parser at all —
  `structurizr-import`'s PlantUML/Mermaid importers merely encode diagram
  text into a render-server URL in an `ImageView` (kroki-shaped, network,
  already deferred by P4-D8). No ceiling exists; the feature needs its
  own scope, so it is an additional feature, not a port.
- Feature: `oxidizr import <FILE> [--format c4plantuml|mermaid|image]
  [--view-key K --view-name N] -o <WORKSPACE>` — hand-rolled macro-subset
  parsers producing model + one view containing exactly the declared
  elements/relationships (macro inventory and fidelity bar in
  `build/PHASE_4.md` P4-D8 + §P4-M2; unknown macros = warning + skip,
  never fail-all). The `image` leg IS the reference port (file →
  `ImageView` data-URI/content-type/title). Decomposition:
  `build/PHASE_4.md` §P4-M2.
- Compatibility: purely additive — new crate `oxidizr-import`, new CLI
  subcommand; output is standard Structurizr JSON the official tooling
  loads (oracle read check gates it); nothing in the contract pipeline
  changes (bless 307×unchanged remains the tripwire). Fidelity bar is
  SEMANTIC (element/relationship sets match the source diagram, like the
  P2 exports), not byte-level — there is no reference output to match.

### F13 — viewer dependency-arrow model editing ✅ (signed off 2026-11-28, questions tool)

- Problem: the viewer edits layout only (P3-D4 editor rule) — arrows can be
  bent but never created, re-targeted, deleted or relabeled; every such
  change means leaving for a text editor. The reference has no GUI editing
  at all, so there is nothing to diverge FROM — pure UX gain (user charter:
  "This can diverge from structurizr, if it makes UX better").
- Feature: create a relationship by dragging from an element's port dot onto
  another element; reconnect an endpoint by dragging the endpoint handle;
  delete the selected relationship with Del; edit description/technology/view
  label in the details panel. Model edits land through invertible
  `ModelCommand`s in a single session history; saves go through the SAME
  shared JSON writer. Blocked for DSL-sourced workspaces (the DSL file is
  never rewritten and the sidecar must not silently become the model of
  record) — plain `.json` sources only. Decomposition + decisions:
  `build/PHASE_6.md` (P6-D2…D8).
- Compatibility: purely additive GUI surface — parity paths untouched
  (bless 307×unchanged stays the tripwire); a saved edited workspace must
  load and re-export cleanly in the official tooling (P6-V2 oracle witness).
  Undo granularity diverges from the official layout editor's per-diagram
  stacks: DEVIATIONS.md D17.

### F14 — viewer help overlay, selection details + markdown docs ✅ (signed off 2026-11-28, questions tool)

- Problem: the viewer documents nothing — no keybinding reference exists,
  hover tooltips truncate, and the model's rich text (descriptions,
  properties, `!docs` markdown sections) is never rendered. The reference
  web UI shows a properties popup; oxidizr's native shell can do it from the
  same model data, better.
- Feature: F1/`?` help overlay rendered from a single headless keybinding
  table (`shortcuts.rs` — the only place bindings are written down); a
  selection details panel (full description, metadata, tags, properties,
  URL, editable edge fields); a documentation window rendering `!docs`
  sections (markdown via `egui_commonmark`, gui-feature-only dep; AsciiDoc
  shown verbatim). Decomposition: `build/PHASE_6.md` P6-D7.
- Compatibility: read-only presentation; no files touched beyond the
  existing save path; none.

### F15 — auto-layout engine framework: dagre default, compound boundaries, pinned tidy, per-view config ✅ (signed off 2026-11-28, questions tool)

- Problem: the MVP `Layered` engine's quality ceiling is structural
  (longest-path ranks, barycenter-only ordering, boundaries not layout
  inputs), Apply destroys manual placements, and dialog parameters are one
  global set — the five `automaticLayout` fields exist per view but nothing
  per-view remembers or writes them.
- Feature: `dagre` (dagre.js port, crates.io, pure Rust, statically
  linked) behind a non-default `dagre` cargo feature of `oxidizr-layout`
  becomes the DEFAULT engine (network-simplex ranks, Brandes-Köpf
  coordinates, COMPOUND graphs — boundaries enter the layout). Engines
  become first-class: feature + availability driven, selectable in the
  viewer dialog and `oxidizr view --layout-engine` (the frozen
  `dot-backend` rides the same framework). Pinned tidy: elements with
  manual (overlay) positions hold their EXACT coordinates while the rest
  lays out around them. Per-view layout params in the viewer cache +
  opt-in "Remember in view" writing the five STANDARD `automaticLayout`
  fields back through the model-edit gate. Decomposition:
  `build/PHASE_7.md`.
- Compatibility: written bytes stay contract-exact — write-back touches
  only the standard fields via the shared writer, plain `.json` sources
  only; no new formats, nothing the official tooling cannot read.


## Ideas (💡 not agreed — sign-off required before implementing)

| # | Idea | Compat analysis |
| --- | --- | --- |
| I5 | ⚠️ Named-argument DSL syntax (`person "Name" technology "..."`) | DANGEROUS: adds grammar the official tool cannot read → files diverge from Structurizr. Rule 2 applies; realistically NOT adoptable unless it provably never appears in output. Keep parked |

(Former I1–I4 were agreed on 2026-11-14 and promoted to F5–F8 above; I5 keeps its number
so old references resolve.)

## Explicitly out of scope (for now, per PLAN.md §Out of scope)

Diagram exporters (PlantUML/Mermaid/…), server/lite/UI functionality, MCP — these are
PARITY features deferred by milestones, not additional features; they belong in a future
roadmap doc when scheduled, not in this registry.
