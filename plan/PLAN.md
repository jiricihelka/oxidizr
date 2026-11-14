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
