# PHASE_4.md — detailed implementation plan: CLI tools, importers, release

Scope per PLAN.md Phase 4 (defaults adopted 2026-11-14): **F3 `diff`, F6 `query`,
F7 `init`, F5 `lsp`, F4 `oracle` user command, inspect-parity**, plus the diagram
**importers moved here from Phase 2** (C4-PlantUML, Mermaid, image→CustomView) and the
**WebSequenceDiagrams exporter deferred here** (the reference CLI does wire it — we
still ship it late since PlantUML sequence output already covers dynamic views), and
**release engineering** (cargo-dist 3-OS binaries, semver + changelog discipline from
v0.1.0).

JSON compat contract unchanged; nothing in this phase may alter bytes written by
`oxidizr-json`.

## P4-D1 Crate & command topology

```
crates/
  oxidizr-import   diagram importers; deps: core, diag, ast(?) for nothing — pure
                   hand-rolled subset lexers, no dsl dep
  oxidizr-lsp      LSP server BINARY; deps: core, ast, dsl, diag, json(fmt via dsl)
  oxidizr-inspect  advisory inspections; deps: core, diag
  oxidizr-cli      gains: diff, query, init, inspect, oracle, import, completions;
                   WSD exporter lands in oxidizr-export
```

- `oxidizr-lsp` is its own binary (tokio-free sync server, see P4-D5); everything else
  is a subcommand of `oxidizr-cli` (clap), global flags inherited (P1-D8), exit-code
  scheme extended: diff adds `1 = differences found` (GNU-diff convention, documented).

## P4-D2 `diff`

- `oxidizr diff <A> <B> [--format human|json] [--ignore-layout] [--quiet]` — both
  inputs auto-detected (DSL/JSON), loaded through the standard P1 pipeline.
- **Identity model** (ids are unstable across edits — never diff on them): model
  elements keyed by canonical name + type; relationships by
  `sourceCanonicalName -> destinationCanonicalName [description]`; people/systems/
  containers/components/nodes/instances handled per type; views by key; styles by
  (kind, tag). ID drift on same-identity items is reported as its own category
  (`idChanged`), never as add+remove.
- Changed items emit field-level diffs (`name`, `description`, `tags`, `technology`,
  `group`, `url`, properties, style fields; geometry per view unless
  `--ignore-layout`).
- `--format json`: stable machine schema — `{ schema: 1, added: [...], removed: [...],
  changed: [{ path, field, before, after }], idChanged: [...] }` with
  `path` = dotted canonical path (`model.softwareSystems[Payment System]`); the schema
  is **our own contract from v0.1** → versioned field `schema`, documented in
  `docs/diff-schema.md`, additive-only changes bump within minor versions.
- Human format: colored ±lines grouped by category; `--quiet` = exit code only.
- Property tests: `diff(a,a)` empty; diff of oracle-relevant workspaces vs
  hand-curated fixture pairs (vendored); `--format json` snapshot.

## P4-D3 `query`

- No new language: the **DSL filter/expression grammar is the query language**
  (agreed). P1 parses that grammar for filtered views; P4 exposes it:
  `oxidizr_dsl::filter::parse(expr) -> oxidizr_core::filter::Predicate` (already the
  type the P3 filtered-view evaluator consumes — one parser, one evaluator, three
  consumers: filtered views, `query`, `inspect`).
- `oxidizr query <WORKSPACE> --filter EXPR [--kind element|relationship|view|all]
  [--format table|json|ids] [-o]`
  - default `table`: `id  kind  canonicalName  [tags…]`; `ids` = one id per line
    (pipeline-friendly); `json` = array of stable objects (same schema-versioning
    discipline as diff).
  - `--filter` applies per `--kind`: elements via `core::filter`; relationships via
    the relationship filter grammar; views by key/name/tags (simple predicate subset —
    cataloged in the same grammar test corpus).
- Tests: evaluator unit matrix (mirrors P3-V5 corner cases), CLI golden against
  vendored workspaces, proptest that any `query --kind element --format ids` output is
  reproducible from the JSON alone (no DSL-only state leaks).

## P4-D4 `init`

- `oxidizr init [DIR] [--template minimal|c4] [--force]` — scaffolds `workspace.dsl`
  (+ `README.md`, `.gitignore`, `.editorconfig`, `.vscode/settings.json` with the
  `oxidizr-lsp` note), all templates `include_str!`-embedded — **zero network, zero new
  deps** (agreed F7 scope).
- Idempotent: existing files skipped with a note unless `--force`; prints the created
  tree + next commands (`oxidizr validate`, `oxidizr export`).
- Template content = the canonical 3-software-system C4 starter using only grammar we
  support (P1 fixture-tested: `init` output must pass `validate` and
  `dsl2json` golden-check — template drift breaks CI, which is the point).

## P4-D5 `lsp`

- Binary `oxidizr-lsp` (stdio LSP). Deps: `lsp-server` + `lsp-types` (rust-analyzer
  lineage, **sync, no tokio** — keeps the lean-deps rule; a tower-lsp choice would
  drag async machinery for zero benefit at DSL file sizes).
- Sync model: **full-document sync** (`textDocument/didOpen|didChange` replaces the
  buffer; DSL files are small; incremental sync is a recorded maybe-later).
- Features (agreed scope):
  - **diagnostics**: push on change — the whole D1 pipeline (multi-error, snippets,
    codes as `codeDescription` links to `docs/diagnostics.md#OXZ-nnnn`), plus core
    validation diagnostics with spans where available;
  - **completion**: scope-aware — the P1-D3 scope-context state machine's
    permitted-token sets are the completion source (keyword + trigger `!`-directives
    + known identifiers: element identifiers for relationship targets, view keys,
    tag names from `styles`); snippets for statement forms (`sys`, `rel`, …);
  - **hover**: keyword docs (embedded docs table) + model hover (element summary:
    type, canonical name, tags, relationship count);
  - **formatting**: full + range via F2 `fmt` (range maps to top-level statement
    granularity — documented behavior);
  - deferred (registered 💡): semantic tokens, rename, code actions.
- Config: `initializationOptions` carries `strict`, `themesDir`, `allowNet`,
  `maxNumberOfProblems`; server never touches network without `allowNet` (D5).
- Testing (all Category A): JSON-RPC transcript harness — in-memory duplex pipes,
  fixture conversations (`expect-test` on message streams); a `lsp-fixture/` corpus of
  edit sequences asserting diagnostic convergence; no editor involved.

## P4-D6 `oracle` user command (F4 graduation)

- `oxidizr oracle gen|verify [--bin PATH] [paths…]` — golden generation/verification
  for users (CI pipelines of fixture authors), sharing the xtask implementation via a
  lib (`oxidizr-cli/src/oracle/` module; xtask stays a dev shim). Binary discovery
  reuses agents/plan/TESTING.md (`$OXIDZR_STRUCTURIZR_BIN` → PATH); the official binary is
  an external prerequisite — slow, goldens stay committed, and oxidizr never downloads
  anything (D5).
- `xtask oracle verify` (CI, Category B job) unchanged; user command adds `--json`
  report mode for pipelines.

## P4-D7 `inspect` (inspection parity)

- Port `structurizr-inspection` (pinned commit: ~35 concrete `Inspection` classes under
  `model/` + `view/`, e.g. `ElementDescriptionInspection`, `DisconnectedElementInspection`,
  `ManualLayoutInspection`, `GeneratedKeyInspection`, `EmptyViewInspection`):
  `oxidizr-inspect` crate, one module per inspection, severity via
  `SeverityStrategy` ported incl. **`PropertyBasedSeverityStrategy`** (workspace
  properties `structurizr.inspections.<InspectionName> = ignored|info|warning|error`
  — exact key format is verification item P4-V2).
- `oxidizr inspect <WORKSPACE> [--severity-threshold warning|error] [--only NAME]…
  [--except NAME]… [--format human|json]` — advisory only: **no effect on the JSON
  contract**; `--severity-threshold error` exit 1 makes it a CI gate.
- Default severities mirrored from Java defaults (catalog in the port PR); snapshot
  tests per inspection against synthetic trigger fixtures; proptest that
  `inspect(validate-clean) == []` for generated valid workspaces (loose coupling:
  inspections ≠ validation).

## P4-D8 Diagram importers (`oxidizr-import`)

- `oxidizr import <FILE> [--format c4plantuml|mermaid|image] [--view-key K --view-name N]
  -o <WORKSPACE.json>` (extension sniff when `--format` absent; kroki deferred — it is
  a network fetch wrapper, D5-adjacent, 💡 later).
- **C4-PlantUML importer**: hand-rolled subset parser for the C4-PlantUML macro
  language as used in the wild: `Person|Person_Ext|System|System_Ext|System_Boundary|
  Container(_Ext)?|Container_Boundary|Component|Person_Boundary|System_Information|…`,
  `Rel(_[A-Za-z]+)?(src,dst,label,tech,tags,…)`, `System_Rel|System_Ext_D…` layout
  hints ignored, `!include`/`!$` preprocessor lines tolerated, `LAYOUT_WITH_LEGEND()`
  tolerated, boundary nesting → grouped elements; produces model + a view containing
  exactly the declared elements/relationships (+ tags from macro tag args). Exact macro
  set = what the reference `structurizr-import` plantuml importer handles **plus**
  vendored-real-world fixes (P4-V3 inventories both); unknown macros → warning + skip
  (never fail-silent, never fail-all).
- **Mermaid importer**: the C4-for-Mermaid subset the reference supports
  (`C4Context|C4Container|C4Component|C4Deployment` bodies: `Person(…`, `System(…`,
  `System_Boundary(…) { … }`, `Rel(…)` / `-->` edges) — scope cataloged from
  reference source first (P4-V3), plain flowchart import is NOT promised.
- **Image importer**: `--format image` wraps an image file in a CustomView
  (`image { … }`-equivalent model entries) — pure metadata, trivially compat-safe.
- **Fidelity bar**: semantic (like P2 exports): the imported workspace must declare
  the same element/relationship sets as the source diagram; tested against reference
  `structurizr-import` test fixtures (vendored) with the reference importer as oracle
  (Category B) + our own goldens (Category A). Gate: **output JSON loads cleanly in
  the reference tool** (oracle read check).
- The **WSD exporter** (deferred from P2) lands here in the same milestone:
  `websequencediagrams` format string, dynamic views only, reuses P2 exporter traits +
  semantic comparator + own goldens.

## P4-D9 `generate system-landscape` (adopted at plan verification, 2026-11-14)

- Port the reference app's `SystemLandscapeGenerator` behind
  `oxidizr generate system-landscape <WORKSPACE> [--format dsl|json] -o <out>`: derive
  a system-landscape workspace from an existing model (copy landscape-relevant people
  + software systems and their relationships into a new workspace with generated
  landscape views), emitted via the existing DSL/JSON writers.
- Deterministic → oracle-verifiable (Category B, sparing, `reference` feature) and
  pure-Rust goldened (Category A). Locked list respected: view/landscape generation
  lives ONLY in this app-layer command, never in parser/library.
- The reference `merge` command (browser-based branch-merge tool, server/branch
  territory) is **explicitly out of scope** — recorded here and in PLAN.md.

## P4-D10 Release engineering (cargo-dist)

- `cargo-dist` (github matrix): x86_64 + aarch64 linux (musl for static-ish linux
  binaries where the toolchain cooperates — else gnu + note), x86_64 + aarch64 macOS,
  x86_64 windows (msvc; `aarch64-pc-windows-msvc` if cheap) → `.tar.gz`/`.zip` +
  `sha256` + install script on GitHub Releases; **binaries**: `oxidizr`,
  `oxidizr-viewer`, `oxidizr-lsp`.
- Metadata & publishing: MIT OR Apache-2.0 everywhere (agents/plan/WORKFLOW.md license policy); library
  crates published to crates.io alongside the dist binaries (metadata complete since
  P1-M0).
- Semver from v0.1.0 (P1-M7): compat contract fixes = patch; CLI/JSON-schema additions
  = minor; breaking = major + migration note (all recorded in DEVIATIONS/ADDITIONAL
  registers as usual). Nightly pin travels in `rust-toolchain.toml`; dist binaries
  free users from it.
- `CHANGELOG.md`: hand-curated Keep-a-Changelog sections per release (no auto-gen
  noise); release checklist doc (`docs/RELEASING.md`: tag → CI green → dist publish →
  changelog + register sync).
- `oxidizr completions <SHELL>` via `clap_complete` (bash/fish/zsh/powershell/elvish).
- CI additions: release job dry-run (`dist --check`), binary smoke test per OS
  (`oxidizr --version`, `dsl2json` on one fixture) — Category A (no reference bin).

## P4-D11 Milestones (ordered, each ends green)

- **M0** `diff` (both formats + schema doc) — first, pure model logic, unblocks
  dogfooding everything after.
- **M1** `inspect` port + severity config; snapshots.
- **M2** `oxidizr-import`: C4-PlantUML → Mermaid → image; WSD exporter; `!docs`/`!adrs`
  importers (P4-D13, retiring the P1 OXZ-5003 deferral); oracle read gate green;
  fixtures vendored with attribution.
- **M3** `query` (filter grammar public API + evaluator already exist from P1/P3) +
  `init` + `completions` + `generate system-landscape` (P4-D9).
- **M4** `lsp` binary: diagnostics → completion → hover → formatting, transcript
  harness; editor smoke notes (vscode, helix, neovim) in docs.
- **M5** `oracle` user command; **P4-D13 carry-ins**: Structurizr-PlantUML + Mermaid
  semantic comparators, `Layered` layout polish; cargo-dist wired + v0.4.0 as the first
  dist release; changelog catch-up through v0.3.0; docs pass; **roadmap complete**.

**RE-ORDERED 2026-11-21 (user directive: "prioritizing LSP functionality"):**
the M4 `lsp` binary moves to the FRONT as **§P4-LSP** (decomposition below,
items L-1…L-7, commits L0…L5); M0 `diff` → M1 `inspect` → M2 importers →
M3 query/init/completions/generate → M5 oracle+carry-ins+dist follow in
their original relative order.

## §P4-LSP — `oxidizr-lsp` decomposition (started 2026-11-21, runs FIRST)

F5 (agreed 2026-11-14), P4-D5. New crate touching existing `oxidizr-dsl`
(scope-replay helper) ⇒ charter decomposition up front. Probes done
2026-11-21 (P4-V5 part 1): `lsp-server 0.10` + `lsp-types 0.97` resolve
and build on the pinned nightly, sync/no-tokio as planned. Seams the
design rides (all pre-existing): `oxidizr_api::open(&Input::Bytes)` =
the exact CLI pipeline over a buffer; `oxidizr_dsl::engine::parse_dsl`;
`Ctx::permitted_tokens()` (P1-D3 scope machine) = the completion
source; `oxidizr_dsl::fmt::format` = lossless printer;
`oxidizr_diag::{Span(byte offsets), Position(1-based, char col),
Code(OXZ-nnnn)}`.

| Item | Deliverable | C |
|---|---|---|
| **L-1** | Crate shape per the viewer precedent (R-2 ritual): `oxidizr-lsp` = **headless lib core** (doc store + feature handlers as pure functions: text+position in, LSP types out — Category A testable) + thin stdio binary; workspace member, lints inherit, `publish = true` (it ships in P4-D10) — **DONE 2026-11-21** | L0 |
| **L-2** | Server loop: stdio `lsp-server` `Connection`, `initialize`/`shutdown`/`exit`, capability set grows with each item; **full-document sync** (`didOpen`/`didChange` full/`didClose`, `syncKind = Full` — files are small, incremental stays 💡); transcript harness = in-memory pipes driving the core loop, `expect-test` on the message stream — **DONE 2026-11-21** | L0 |
| **L-3** | Diagnostics on open/change: buffer → `api::open(Input::Bytes)` → map every diagnostic to `lsp_types::Diagnostic` — ranges from BYTE OFFSETS recomputed against the buffer text (LSP columns are **UTF-16 code units**; diag `Position.col` counts chars — the mapper is a tested pure fn), severity map, `code` = `OXZ-nnnn` + `codeDescription` → `docs/diagnostics.md#OXZ-nnnn`, `maxNumberOfProblems` cap (default 100); JSON buffers validate via `json2json` — **DONE 2026-11-21** (`.json` URIs route in `diagnostics.rs` since `Input::Bytes` never carries an extension; snippet spans located by `find`, unlocatable → neutral 0:0; `didClose` ends with an EMPTY publish) | L1 |
| **L-4** | Completion: new `oxidizr-dsl` pub helper replays tokenize+scope to a cursor offset (returns the `Ctx` + preceding identifiers); items = `permitted_tokens()` + `!`-directives + contextual identifiers (element/relationship id targets, view keys, style tags) + statement snippets (`sys`, `rel`, …) | L2 |
| **L-5** | Hover: embedded keyword-doc table + model hover (kind, canonical name, technology, tags, relationship counts) from the LAST SUCCESSFUL parse of that doc | L3 |
| **L-6** | Formatting: full-document `textDocument/formatting` = one TextEdit from `fmt::format`; `rangeFormatting` maps to top-level-statement granularity (documented); buffers WITH parse errors still format — the P1-M6b printer is lossless over the CST | L4 |
| **L-7** | Config & close: `initializationOptions` `{strict, themesDir, allowNet=false, maxNumberOfProblems}` (no network ever without `allowNet`, D5); `oxidizr-lsp --version`; `docs/lsp-editors.md` smoke notes (vscode/helix/neovim); milestone close = full e2e tier + bless 307×unchanged + PROGRESS/RESUME | L5 |

Deferred (already 💡-registered in P4-D5): semantic tokens, rename,
code actions, incremental sync. Risk R-3 (LSP UX surprises) mitigated by
full-sync + the offset→UTF-16 mapper being pure + transcript tests.

## P4-D12 Verification list

- P4-V1 diff identity rules vs real-world edit sequences (identifier/canonical-name
  stability limits) — refine `idChanged` taxonomy before freezing `schema: 1`.
- P4-V2 exact inspection severity property keys + Java default severities.
- P4-V3 macro/syntax inventory: reference plantuml/mermaid importer accepted forms vs
  vendored real-world samples (drives importer scope).
- P4-V4 filter/expression grammar full token inventory (what `query` can promise)
  from `ExpressionBuilder`-class sources.
- P4-V5 `lsp-server` version + stdio framing behavior under fast edits.
- P4-V6 cargo-dist musl/aarch64 feasibility with the pinned nightly; signing deferred
  (unsigned v1 — documented, not deviating from anything).
- P4-V7 (ex-P1-V9) ADR status/date parsing corners per tool format (MADR v2/v3,
  log4brains front matter) — before the importer code (P4-D13).
- P4-V8 (ex-P1-V6) `!docs`/`!adrs` exclude-pattern semantics (glob vs path prefix) —
  verify with reference before trusting globset config (P4-D13).

## P4-D13 Scope carried in from the 2026-11-14 rebalance

Three cost-trimming deferrals land here (decided via questions tool; the phase plans
they leave are annotated in place):

1. **`!docs` / `!adrs` importers** (from P1-M5): markdown documentation importer +
   all 4 ADR tool formats (adr-tools, MADR, log4brains, structurizr-format) + asciidoc
   passthrough + `globset` exclude patterns. The P1 parser already parses the statement
   form, so this replaces the OXZ-5003 "deferred" diagnostic with real import — no
   grammar work. Retires the deferred-corpus exclusion in `testdata/README.md`;
   P1's full-grammar promise completes **here**. Vendored real-world fixture corpora +
   reference importer as oracle (Category B). New deps: `globset` (returns).
   Verification: P4-V7 (ex-P1-V9: ADR status/date parsing corners, MADR v2/v3,
   log4brains front matter) and P4-V8 (ex-P1-V6: exclude-pattern semantics glob vs path
   prefix — verify before trusting globset config).
2. **Semantic comparators for Structurizr-PlantUML + Mermaid** (from P2-D9): same
   `xtask/exportcmp` framework as the P2 C4-PlantUML comparator (whose grammar table is
   the head start for PlantUML); closes the fidelity window for the two dialects that
   P2 gated with own-goldens + render checks only.
3. **`Layered` layout engine polish** (from P3-D5 MVP): simplex/tightening layering
   refinement, priority-method coordinates, smarter boundary contraction, edge-channel
   routing. Determinism property + benches already in place from P3; quality upgrades
   must preserve both (regression-gated by existing suites).

## Risks

- R-1 importer scope creep — `.puml`/mermaid dialects are lakes (mitigation:
  reference-importer scope as the ceiling, warning-and-skip, semantic bar).
- R-2 diff schema regret (mitigation: explicit `schema` field, schema doc frozen at
  M0, additive-only policy).
- R-3 LSP UX surprises (edits mid-parse, huge files) (mitigation: full-sync model,
  diagnostic caps from P1, transcript tests).
- R-4 inspection port drift from Java severities (mitigation: catalog + per-inspection
  snapshots against the oracle where checkable).
- R-5 dist matrix pain (mitigation: M5 buffer; plain `cargo build` instructions remain
  the fallback story at all times).
