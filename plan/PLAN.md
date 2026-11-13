# PLAN.md — phase roadmap

Four phases, each independently shippable. **Every phase ends with the same compat
gate:** the golden harness (F4 oracle as `xtask`, Docker-pinned official tooling) shows
byte-identical JSON output vs the reference, and the read-corpus (legacy/unknown-field
workspaces) loads. Deviations follow `DEVIATIONS.md`; features follow
`ADDITIONAL_FEATURES.md` (F-numbers referenced below).

## Phase 1 — Parsers & writers (the compatibility core)

Goal: `oxidizr` can losslessly read every Structurizr input and write byte-perfect JSON.

- `oxidizr-core`: immutable domain model + canonical names, tag stacks, id counter
  (Locked behaviors).
- `oxidizr-dsl`: CST-based parser for the full v6 grammar (DSL.md), incl. `!docs`/`!adrs`
  markdown/ADR importers (needed for full DSL compat); JVM-only directives rejected
  cleanly (D2); `${VAR}` pre-pass; portability/`structurizr.dsl` retention rules.
- `oxidizr-json`: reader (lenient, unknowns ignored; optional capture for `--keep-unknown`
  = D3) and writer (alphabetical keys, NON_EMPTY, UTC-Z dates, 2-space indent).
- Diagnostics engine (D1): multi-error, column/snippet, suggestions; `--strict`.
- CLI MVP: `dsl2json`, `json2json`, `validate`.
- Harness from day one: F4 oracle xtask (golden generation/verification) + `testdata/`
  vendored from FIXTURES.md.
- Early-pulled features: **F2 `fmt`** and **F1 `reconcile`** (both need only the P1 CST).
- Exit: all fixture DSL parses; written goldens byte-match; petclinic + `backwardsCompatibility/`
  read cleanly; fmt invariant and reconcile remap tests green.

## Phase 2 — Converters (diagram formats)

Goal: workspace in → standard diagram text out, and simple diagrams in.

- Exporters: PlantUML (Structurizr dialect + C4-PlantUML, light/dark), Mermaid, theme
  export (`styles` → theme.json). Optional later: WebSequenceDiagrams (dynamic views),
  Graphviz DOT (reference CLI doesn't wire it either).
- Importers: C4-PlantUML and Mermaid (C4-stereotype) diagram files → model + view;
  image → CustomView.
- Theme resolution stays offline-first (D5): cache/`--themes-dir`, relative-icon
  resolution against theme base.
- No layout engine yet: exported text formats self-layout.
- Exit: exporter output matches reference on goldens (semantic diff; header/version lines
  whitelisted); imported diagrams produce workspaces the reference tool accepts.

## Phase 3 — GUI (native desktop, viewer + layout editing)

Goal: single-binary desktop app to see and tune diagrams — no browser, no Docker, no Java.

- Toolkit decision needed at phase start: **egui vs iced** (open question O1).
- Renderer: all view types (landscape/context/container/component/dynamic/deployment/
  filtered + custom/image), styling incl. shapes, borders, icons, metadata, terminology,
  light/dark color schemes; default theme bundled.
- Auto-layout engine decision (open question O2): external `dot` vs ELK vs pure-Rust —
  result is only final `{x,y}`/points written into standard JSON.
- Layout editing: drag elements/waypoints, autosave into workspace JSON geometry; saves
  must stay readable by official tooling (compat gate applies); integrate F1 remap so
  edited layouts survive later DSL edits.
- Exit: opens vendored real-world workspaces (big-bank-plc, spring-petclinic, aws) and
  renders correctly vs reference styling; drag-edit → save → reference tool round-trips
  the layout without complaint.

## Phase 4 — Other CLI tools

Goal: the ergonomic registry lands as polished commands.

- **F3 `diff`**, **F6 `query`**, **F7 `init`**, **F5 `lsp`**.
- **F4** graduates from xtask to a user-facing `oracle` command (Docker required only here).
- `inspect`-parity: advisory checks (inspection module equivalent) — not part of the
  JSON contract, useful for CI; severity/config model copied from reference.
- Exit: each command golden/property-tested; `--help` UX reviewed; completion scripts.

## Out of scope for this roadmap

Server/lite/playground/web-UI equivalents, MCP server, static-site/PNG/SVG export,
encrypted workspaces, cloud push/pull. Revisit only if a phase makes them cheap.

## Open questions (own decision entries when due)

- O1 GUI toolkit: egui vs iced (Phase 3 start).
- O2 Layout engine: `dot` subprocess vs ELK vs Rust port (Phase 3 start).
- O3 C4-PlantUML dialect/version targeting for exporter parity (Phase 2 start).
