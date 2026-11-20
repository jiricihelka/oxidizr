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
