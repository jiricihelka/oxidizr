# PLAN.md — phase roadmap

Four phases, each independently shippable. **Every phase ends with the same compat
gate:** the golden harness (F4 oracle as `xtask`, invoking the official reference binary
via `$OXIDZR_STRUCTURIZR_BIN`/PATH — slow, so goldens are committed and oracle runs are
sparing, behind the default-off `reference` feature) shows
byte-identical JSON output vs the reference, and the read-corpus (legacy/unknown-field
workspaces) loads. Deviations follow `DEVIATIONS.md`; features follow
`ADDITIONAL_FEATURES.md` (F-numbers referenced below).

## Phase 1 — Parsers & writers (the compatibility core)

Goal: `oxidizr` can losslessly read every Structurizr input and write byte-perfect JSON.

- `oxidizr-core`: immutable domain model + canonical names, tag stacks, id counter
  (Locked behaviors).
- `oxidizr-dsl`: CST-based parser for the full v6 grammar (DSL.md); doc/ADR importers
  for `!docs`/`!adrs` **deferred to Phase 4** (D7: statement parsed, import deferred
  with OXZ-5003 until then); JVM-only directives rejected cleanly (D2); `${VAR}`
  pre-pass; portability/`structurizr.dsl` retention rules.
- `oxidizr-json`: reader (lenient, unknowns ignored; optional capture for `--keep-unknown`
  = D3) and writer (alphabetical keys, NON_EMPTY, UTC-Z dates, 2-space indent).
- Diagnostics engine (D1): multi-error, column/snippet, suggestions; `--strict`.
- CLI MVP: `dsl2json`, `json2json`, `validate`.
- Harness from day one: F4 oracle xtask (golden generation/verification) + `testdata/`
  vendored from FIXTURES.md.
- Early-pulled features: **F2 `fmt`** and **F1 `reconcile`** (both need only the P1 CST).
- Exit: all fixture DSL parses (minus the deferred docs/ADR corpus — D7); written
  goldens byte-match; petclinic + `backwardsCompatibility/`
  read cleanly; fmt invariant and reconcile remap tests green.

## Phase 1T — E2E regression & benchmark suite (interlude, added 2026-11-17)

Goal: the safety net for the post-replica era — hundreds of close-to-real-world
workspaces asserted as use-cases (valid compiles to the expected shape; invalid
errors with the expected OXZ code set), in-process via a new stable `oxidizr-api`
seam, blessable manifests (no bytes, no messages), shared corpus feeding the
benches too. Runs between v0.1.0 and Phase 2; no shipped-behavior change, no tag.
Details: `build/PHASE_1T.md`.

## Phase 2 — Converters (diagram export)

Goal: workspace in → standard diagram text out.

- Exporters: PlantUML (Structurizr dialect + C4-PlantUML, light/dark), Mermaid, theme
  export (`styles` → theme.json). Deferred to Phase 4: WebSequenceDiagrams (the
  reference CLI does wire it; PlantUML sequence output covers dynamic views meanwhile),
  image→CustomView. Never: Graphviz DOT/ilograph (library-only in the reference).
- Diagram importers (C4-PlantUML, Mermaid) are **Phase 4** (decision 2026-11-14 —
  import is rarer than export; halves Phase 2).
- Fidelity bar: **semantic parity + render checks** (oracle generates reference
  exports; structure-based diff + render smoke tests), not byte-parity — the JSON
  contract is the byte-parity contract. Comparator automation covers C4-PlantUML in
  P2; the other dialects' comparators carry into P4-D13 (own goldens + render checks
  gate them meanwhile).
- Theme resolution stays offline-first (D5); this phase introduces `--allow-net`.
- No layout engine yet: exported text formats self-layout.
- Exit: `build/PHASE_2.md` gates (semantic oracle diff green, render checks, our own
  regression goldens). Details: `build/PHASE_2.md`.

## Phase 3 — GUI (native desktop, viewer + layout editing)

Goal: single-binary desktop app to see and tune diagrams — no browser, no Docker, no Java.

- Toolkit: **egui/eframe (wgpu)** — O1 closed 2026-11-14.
- Renderer: all view types (landscape/context/container/component/dynamic/deployment/
  filtered + custom/image), styling incl. shapes, borders, icons (PNG + SVG/usvg),
  metadata, terminology, bundled font; light/dark follow-OS.
- Auto-layout: own deterministic layered engine behind a `LayoutEngine` trait —
  **MVP scope in P3** (longest-path layering, barycenter ordering, simple coordinates),
  quality polish carried to P4-D13; optional `dot-backend` cargo feature (subprocess,
  off by default) — O2 closed.
  Result is only final `{x,y}`/points written into standard JSON.
- Layout editing: drag elements/waypoints with undo/redo, save into workspace JSON
  geometry; saves must stay readable by official tooling (compat gate applies); file
  watching with live reload preserving layout (F1 remap engine moves to `core`).
- Exit: opens vendored real-world workspaces (big-bank-plc, spring-petclinic, aws) and
  renders correctly vs reference styling; drag-edit → save → reference tool round-trips
  the layout without complaint. Details: `build/PHASE_3.md`.

## Phase 4 — Other CLI tools

Goal: the ergonomic registry lands as polished commands; roadmap completes.

- **F3 `diff`**, **F6 `query`** (DSL filter grammar as the query language), **F7
  `init`**, **F5 `lsp`** (diagnostics/completion/hover/formatting), **`inspect`**
  parity (~35 reference inspections, property-based severity config).
- **F4** graduates from xtask to a user-facing `oracle` command (reference binary is
  an external prerequisite; oxidizr never downloads it).
- Importers (from P2): C4-PlantUML, Mermaid, image→CustomView; plus the deferred
  WebSequenceDiagrams exporter and the **`generate system-landscape`** port
  (reference-app command parity, adopted 2026-11-14).
- **Carry-ins from the 2026-11-14 rebalance** (`build/PHASE_4.md` P4-D13):
  `!docs`/`!adrs` markdown + ADR importers (retiring D7), Structurizr-PlantUML +
  Mermaid semantic comparators, `Layered` layout quality polish.
- Release engineering: cargo-dist 3-OS binaries, semver + hand-curated changelog from
  v0.1.0; `completions`.
- Exit: each command golden/property-tested; `--help` UX reviewed; first cargo-dist
  release. Details: `build/PHASE_4.md`.

## Phase 4T / 4R / 4D — hardening interludes, then the release tail (added 2026-11-22)

Goal: ship v0.4.0 on a hardened base. Ordered interludes between the Phase 4
feature milestones (§P4-M5 E-1…E-3) and the release tail (E-4/E-5, which move to
the very end): **P4T** — every Phase 4 feature gets a named test in every suite,
the feature × suite matrix becomes the checked coverage artifact
(`build/PHASE_4T.md`); **P4R** — small-scope idiomatic-Rust refactors with the
public API churn allowed (byte contract/CLI frozen; 2R precedent),
`build/PHASE_4R.md`; **P4D** — documentation reset: short human-first text,
no dev-process or Java-reference mentions in human-facing docs, README per
directory, root README as user documentation (`build/PHASE_4D.md`). The
`v0.4.0` tag lands after P4D closes; roadmap completion = P4D + release tail.

## Phase 5R — large-scale idiomatic-Rust refactor → v0.5.0 (added 2026-11-25)

Goal: structural idiomaticity on the released tree — giant-file module
decomposition, error-taxonomy redesign (OXIDIZR-COINED texts may improve,
reference-ported stay verbatim), typed ids/newtypes in the engines,
visibility minimization, allocation removal with criterion evidence, crate
topology reviewed (default KEEP). Successful-output bytes, CLI grammar, exit
classes and OXZ codes stay frozen; the Rust API churns freely with every
line recorded in the changelog `[Unreleased]`. Closes with the v0.5.0
release ritual + local tag. Decisions + waves: `build/PHASE_5R.md`.

## Phase 6 — GUI improvements: arrow editing + help texts (added 2026-11-28)

Goal: the viewer becomes a real diagram EDITOR and finally documents
itself. Dependency arrows become first-class editable objects — create by
port-drag, reconnect by endpoint handle, delete with Del, relabel in a
details panel — through invertible model commands on a single session
history, saved via the shared JSON writer, gated to plain `.json` sources
(the DSL file is never rewritten). Help texts land as an F1 keybinding
overlay (one headless table), a selection details panel with markdown
descriptions (`egui_commonmark`, gui-feature-only), and a `!docs`
documentation window. P3-D4 editor rule amended; D17 registered;
F13+F14 signed off. No tag at close. Decisions + waves: `build/PHASE_6.md`.

## Out of scope for this roadmap

Server/lite/playground/web-UI equivalents, MCP server, static-site/PNG/SVG export,
encrypted workspaces, cloud push/pull, and the `merge` command (browser-based
branch-merge tool; server/branch territory — recorded in `build/PHASE_4.md` P4-D9).
Revisit only if a phase makes them cheap.

## Open questions

Resolved 2026-11-14 (decisions in the phase plans):

- O1 GUI toolkit → **egui/eframe (wgpu)** — `build/PHASE_3.md` P3-D2.
- O2 Layout engine → **own deterministic layered engine + optional `dot-backend`
  cargo feature** — `build/PHASE_3.md` P3-D5.
- O3 C4-PlantUML dialect/version → **mirror the reference at the pinned commit**:
  `c4plantuml.stdlib` property (default true) selects `!include <C4/…>` stdlib form vs
  raw master-URL includes — `build/PHASE_2.md` P2-D6.
