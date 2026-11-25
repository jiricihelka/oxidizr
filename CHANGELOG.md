# Changelog

All notable changes to oxidizr are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/). The released CLI
grammar, JSON byte output and diagnostic codes are the product contract: a
change to any of those is called out explicitly below. The Rust APIs are not
yet a stability contract — breaking Rust-level changes are listed under
**Changed → Rust API** so they can be found.

## [Unreleased]

Rust-API churn lands here as it happens (each breaking-ish change gets its
line in the same commit); the section is cut into the next version at its
release.

### Fixed

- Two user-visible messages dropped their citations of internal planning
  documents: the un-implemented export format text (`OXZ-5007`) and the
  oracle "binary not found" text.

### Changed

- Rust API (the large-scale idiomatic-Rust refactor; the CLI grammar,
  output bytes and diagnostic codes are unchanged):
  - oxidizr-api: `generate::pathfilter::PathFilter::compile` now fails with
    the new `PatternError` error type instead of a bare `String`; the
    reason text (decorated into `OXZ-5010`) is unchanged (R1)
  - oxidizr-core: removed the never-constructed identity newtypes
    `ids::ElementId`/`ids::RelationshipId` (the model has always keyed on
    `Id` alone, matching the official shared element/relationship counter)
    and the unused parser-default constants
    `DEFAULT_WORKSPACE_NAME`/`DEFAULT_WORKSPACE_DESCRIPTION` (the parser
    applies those defaults inline); root re-exports trimmed to the names
    consumers import (R5)
  - oxidizr-export: removed the never-implemented `WorkspaceExporter` trait
    (the whole-workspace `json`/`theme` path dispatches inline through
    `ExportFormat::export`); root re-exports trimmed to the names consumers
    import (R5)
  - oxidizr-viewer: removed the never-called `Overview::bounds`/
    `Overview::map` accessors; the module tree is trimmed to the visibility
    its consumers reach (the headless lib core is crate-internal, the `gui`
    shell is its only non-test consumer) (R5)
- The stable-toolchain floor (`rust-version`) rises 1.85 → 1.88: stabilized
  let-chains and slice `as_chunks` are adopted across the workspace.
  Shipped behavior, outputs and the CLI are unchanged.

## [0.4.0] - 2026-11-23

Phase 4: workspace tooling beyond conversion — compare, inspect, import,
query, language server, reference-oracle checks — plus hardening, an
idiomatic-Rust pass and packaged release binaries.

### Added

- `oxidizr diff A B [--format human|json] [--ignore-layout] [--quiet]` —
  compare two workspaces. A `.dsl` file and its JSON twin diff empty:
  provenance stamps are never compared, tags compare as the reference
  effective view. Exit 1 when they differ. JSON report schema documented in
  `docs/diff-schema.md`.
- `oxidizr inspect <WS> [--severity-threshold …] [--only T] [--except T …]
  [--format human|json]` — advisory model/view inspections (a port of the
  official Structurizr inspection engine, messages included). Exit 1 when
  anything is reported. Schema: `docs/inspect-schema.md`.
- `oxidizr import` — build a fresh workspace from a C4-PlantUML source, a
  Mermaid diagram, or a rendered diagram image (model only).
- `!docs` and `!adrs` DSL statements — import documentation and architecture
  decision records from disk (Markdown/AsciiDoc; AdrTools, Madr,
  Log4brains and Structurizr-native decision formats).
- `export -f websequencediagrams` — WebSequenceDiagrams export for system
  landscape sequence views, byte-parity with the reference export.
- `oxidizr query <WS> --filter EXPR [--kind …] [--format table|ids|json]` —
  query people, groups, systems, containers, components and relationships
  with the DSL's own item-expression grammar (`element.tag==Web`,
  `user->system`, …). Schema: `docs/query-schema.md`.
- `oxidizr init [DIR] [--template minimal|c4] [--force]` — scaffold a
  starter workspace project; idempotent.
- `oxidizr completions <shell>` — shell-completion script for bash, zsh,
  fish and friends.
- `oxidizr generate system-landscape <1..N> [--include R] [--exclude R]
  [--relationships first|all]` — derive the system landscape of one or more
  workspaces (port of the reference landscape generator).
- `oxidizr oracle gen|verify [PATHS…] [--bin PATH] [--goldens DIR] [--json]`
  — generate and verify JSON goldens against a local official `structurizr`
  binary (never downloaded; `--bin` or `$OXIDZR_STRUCTURIZR_BIN`).
- `oxidizr-lsp` — a language server for the Structurizr DSL: diagnostics,
  completion, hover, formatting, `initializationOptions`. Editor setup in
  `docs/lsp-editors.md`.
- `oxidizr view [FILE] [--theme …]` — open the native viewer from the main
  binary (with the `gui` feature; without it, the help surface is unchanged
  and the command prints a rebuild hint).
- The viewer lays out un-positioned views on display (the web-app
  behaviour), shows the view list grouped by view kind, uses roomier
  auto-layout separations, pans with the right button, and remembers the
  per-view camera in a `<stem>.oxidizr.json` viewport sidecar (F11).
- Packaged releases: prebuilt binaries for
  x86_64/aarch64-unknown-linux-gnu, x86_64/aarch64-apple-darwin and
  x86_64-pc-windows-msvc with SHA-256 checksums and shell/PowerShell
  installers, published to the GitHub Release by CI on each tag; a per-OS
  smoke run checks `--version` and a `dsl2json` conversion of every release.
- More diagnostics and help: the diagnostics catalogue lives in
  `docs/diagnostics.md`.

### Changed

- Rust API (the idiomatic-Rust refactor pass; the CLI grammar, output bytes
  and diagnostic codes are unchanged):
  - oxidizr-import: `ImportFormat` + typed `ImportFormat::sniff` constructor
    now LIVE HERE (moved out of `oxidizr-api`'s private `sniff_format`);
    `oxidizr_api::ImportFormat` stays valid as a re-export,
    `oxidizr_api::pipeline::ImportFormat` is GONE (R2, item 1)
  - oxidizr-api: `Request::Diff`/`Request::Inspect` payload `json: bool` →
    typed `format: DiffFormat` / `format: InspectFormat` (new api enums,
    re-exported like the `Query` ones); `oxidizr_api::DiffFormat`/
    `InspectFormat` were the CLI's clap enums — those are renamed
    `DiffCmdFormat`/`InspectCmdFormat` in `oxidizr-cli` (grammar frozen: the
    `--format human|json` values are unchanged) (R3, item 1)
  - oxidizr-api: `ApiOptions` field rework — `strict`/`keep_unknown` moved
    into a nested `parse: ParseFlags` struct (the `struct_excessive_bools`
    allow is GONE; `env`/`stdin`/`allow_net`/`themes_dir`/`color` keep their
    names); CLI/LSP/e2e/tests rewired, no behavior change (R3, item 2)
- `Layered` auto-layout: edges sharing a channel now fan into lanes, so
  parallel edges no longer overlap (deterministic at any scale); the
  documented rank assignment is proven the tightest feasible one.
- Human-facing documentation was rewritten short and human-first: README,
  crate READMEs, help texts and `docs/`.
- Workspace version raised `0.1.0` → `0.4.0` to match the release history
  (every crate carries the workspace version).

### Fixed

- `fmt` is idempotent again on files starting with a byte-order mark.
- `generate system-landscape` path filters no longer blow up exponentially
  on zero-width regex assertions (`$+$+…`).
- Live reload in the viewer also reacts to file writes on Windows (the
  backend there reports a generic modify event).

## [0.3.0] - 2026-11-20

Phase 3: the native viewer and the layout engine.

### Added

- `oxidizr-viewer` — a native viewer window: open `.dsl`/`.json`, all nine
  view kinds, editing with undo and `Ctrl+S` JSON save (geometry sidecar
  rule), live reload with reconcile, minimap and quick-nav, SVG and PNG
  export with key, light/dark themes, Graphviz `dot` backend via
  `$OXIDZR_DOT_BIN`.
- `oxidizr-layout` — the `Layered` auto-layout engine (the viewer's layout
  for views without stored geometry), with a committed benchmark baseline.

## [0.2.0] - 2026-11-18

Phase 2: the exporters.

### Added

- `oxidizr export` — `theme` and `json` formats plus diagram formats
  `plantuml`, `c4plantuml` (light and dark) and `mermaid`.
- Theme infrastructure: element/relationship styles, a `ThemeStore`,
  `--themes-dir` resolution.
- Golden corpora and oracle verification for exporter output (JSON, diagram
  and theme sets byte-compared against the official binary), render smokes,
  export benchmarks and a panic-fuzz suite.

## [0.1.0] - 2026-11-16

Phase 1: the DSL/JSON toolchain core.

### Added

- `oxidizr` commands `dsl2json`, `json2json`, `validate`, `fmt` and
  `reconcile`.
- The byte-parity JSON contract: standard Structurizr JSON in and out,
  byte-comparable where the format defines it.
- Fuzz targets and benchmark baselines for the parser/serializer.
