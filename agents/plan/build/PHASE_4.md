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
  ~~`oxidizr_dsl::filter::parse(expr) -> oxidizr_core::filter::Predicate`
  (already the type the P3 filtered-view evaluator consumes — one parser,
  one evaluator, three consumers: filtered views, `query`, `inspect`)~~
  **CORRECTED 2026-11-21** — no `Predicate` type exists; the engine's
  `ExpressionParser` port (`engine/expr.rs`) IS the evaluator and is
  reused via a public `oxidizr_dsl::query` module (§P4-M3 preamble).
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
  properties `structurizr.inspection.<lowercased dotted type path>` =
  `ignored|info|warning|error` — verified via P4-V2 2026-11-21, see the §P4-M1
  decomposition; the earlier `structurizr.inspections.<Name>` guess was wrong).
- `oxidizr inspect <WORKSPACE> [--severity-threshold warning|error] [--only NAME]…
  [--except NAME]… [--format human|json]` — advisory only: **no effect on the JSON
  contract**; `--severity-threshold error` exit 1 makes it a CI gate.
- Default severities mirrored from Java defaults (catalog in the port PR); snapshot
  tests per inspection against synthetic trigger fixtures; proptest that
  `inspect(validate-clean) == []` for generated valid workspaces (loose coupling:
  inspections ≠ validation).

## P4-D8 Diagram importers (`oxidizr-import`)

> **REVISED by P4-V3 (2026-11-21)**: no reference model-parser ceiling
> exists — the C4-PlantUML/Mermaid→model importers ship as agreed
> additional feature **F12**; the `image` leg is the reference port
> (`ImageView` data URIs); full findings + decomposition in §P4-M2.

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
| **L-4** | Completion: new `oxidizr-dsl` pub helper replays tokenize+scope to a cursor offset (returns the `Ctx` + preceding identifiers); items = `permitted_tokens()` + `!`-directives + contextual identifiers (element/relationship id targets, view keys, style tags) + statement snippets (`sys`, `rel`, …) | L2 — **DONE 2026-11-21** (helper = `engine::cursor_scope` → purpose-shaped `CursorScope`, engine types stay `pub(crate)` per 2R R-dsl-09; ONLY logical lines ending strictly BEFORE the cursor are fed — the cursor's own line never; D1 recovery mirrored, `ParseOptions::fragment()` + CLOSED env ⇒ FS-free & deterministic; register inventory via `all_identifiers()` drops auto-UUID keys; `DIRECTIVES` excludes the JVM-only `!plugin`/`!script`; snippets `per`/`sys`/`rel` gated on the keyword they insert — the Model scope has NO `relationship` keyword, so `rel` gates on `->`) |
| **L-5** | Hover: embedded keyword-doc table + model hover (kind, canonical name, technology, tags, relationship counts) from the LAST SUCCESSFUL parse of that doc | L3 — **DONE 2026-11-21** (`hover.rs`: token = quoted-string CONTAINING the cursor, else word run with `!`-prefix; model card beats keyword card; identifier via `structurizr.dsl.identifier` beats name; `Server::last_good` per URI — replaced ONLY by a clean pipeline run (`diagnostics::workspace`: open ok AND empty log), kept across failing reparses, dropped on `didClose`; unopened doc / nothing hoverable ⇒ JSON null, malformed params ⇒ `InvalidParams`; DSL positional strings are name+DESCRIPTION, deployment trees need a `deploymentEnvironment` wrapper, `infrastructureNode` is positional-only, qualified `s.c` refs need `!identifiers hierarchical` — all pinned by fixtures) |
| **L-6** | Formatting: full-document `textDocument/formatting` = one TextEdit from `fmt::format`; `rangeFormatting` maps to top-level-statement granularity (documented); buffers WITH parse errors still format — the P1-M6b printer is lossless over the CST | L4 — **DONE 2026-11-21** (`formatting.rs`: ONE whole-buffer TextEdit, EMPTY array when canonical or `.json` — the byte contract never sees the DSL printer; `rangeFormatting` WIDENS to the top-level statement(s) touched — documented deviation from per-statement wording: canonical style is a global re-print (indent = brace depth from col 0, blank runs compacted), a mid-statement edit would disagree with the next full format; cursor in top-level trivia ⇒ NEXT statement, trailing ⇒ PREVIOUS; statement walk over the lossless CST, brace delta counted from `TokenBare` ONLY — quoted values/`"""`/comments never move the depth; stray closers clamp at 0, braces open at EOF end the statement at buffer end; `FormattingOptions` read but NOT honored — canonical style is the printer's own contract; capabilities `OneOf::Left(true)`; unopened doc ⇒ EMPTY array, malformed params ⇒ `InvalidParams`) |
| **L-7** | Config & close: `initializationOptions` `{strict, themesDir, allowNet=false, maxNumberOfProblems}` (no network ever without `allowNet`, D5); `oxidizr-lsp --version`; `docs/lsp-editors.md` smoke notes (vscode/helix/neovim); milestone close = full e2e tier + bless 307×unchanged + PROGRESS/RESUME | L5 — **DONE 2026-11-21** (`config.rs`: hand-rolled total parse over `serde_json::Value` — field-by-field, wrong-typed/unknown keys ignored, defaults NEVER fail `initialize`; the three CLI flags ride `ApiOptions` on every pipeline run, `allowNet` default OFF and the server adds no network of its own (D5); `maxNumberOfProblems` caps `publishDiagnostics` (0 literal, negative/fraction ⇒ default 100); `--version`/`-V` pinned by `tests/version.rs` (`CARGO_BIN_EXE` — the bin is own-package); `docs/lsp-editors.md` capability table + options + helix/neovim/vscode snippets + smoke checklist. Close: full e2e tier green + `e2e bless` 307×unchanged) |

**§P4-LSP CLOSED 2026-11-21** — L0 `a38f905`, L1 `e11271f`, L2 `fedd7aa`,
L3 `e13af48`, L4 `16d9ab5`, L5 (this commit). Milestone close: full e2e
tier (`OXIDZR_E2E_FULL=1`) green + `cargo xtask e2e bless` 307×unchanged
+ `cargo xtask ci` green; the LSP never touched `oxidizr-json` bytes.

Deferred (already 💡-registered in P4-D5): semantic tokens, rename,
code actions, incremental sync. Risk R-3 (LSP UX surprises) mitigated by
full-sync + the offset→UTF-16 mapper being pure + transcript tests.

## §P4-M0 — `diff` decomposition (started 2026-11-21; items D-1…D-4, commits M0a…M0d; **CLOSED 2026-11-21** — all four items DONE, `cargo xtask ci` green, full e2e tier green, bless 307×unchanged; milestone log `PROGRESS.md`)

Pure model logic over two loaded `Workspace`s — no `oxidizr-json` byte
contact, no new crate (P4-D1: `diff` is an `oxidizr-cli` subcommand; the
BODY lives in `oxidizr-api` per the T0-d invariant "bodies in the API").
The identity engine rides EXISTING seams: `WorkspaceBuilder::adopt` +
`canonical_name_full` / `relationship_canonical_name` (the reconcile
precedent), `canonical::relationship` for the relationship key,
`ViewSet::all_views()` traversal order, `Styles` vectors.

| Item | Deliverable | C |
|---|---|---|
| **D-1** | `oxidizr-api/src/diff.rs`: identity model per P4-D2 — elements keyed by canonical name (the `Person://X` URI prefix IS the type), relationships by `relationship_canonical_name` (`A -> B [description]`), views by key, styles by (kind, tag); SAME identity with a different id ⇒ `idChanged`, NEVER add+remove. Field-level compare (description, tags, technology, group, url, properties per-key, + workspace metadata); views compare base + content ids RESOLVED to canonical names, geometry fields gated by `ignore_layout`; `Diff` as pure data, sorted stable (`path`, `field`); property: `diff(a, a)` EMPTY for every fixture — **DONE 2026-11-21** (10 unit tests pinned on a hand-built fixture: rename = identity pair incl. the relationship whose key embeds the name, consistent id shift = `idChanged` only, `ignore_layout` gate, positional `#N` pairing for duplicate canonical names) | M0a |
| **D-2** | `--format json`: the frozen SCHEMA 1 document `{schema:1, added:[{path,kind}], removed:[…], changed:[{path, field, before, after}], idChanged:[{path, before, after}]}` with dotted canonical paths (`model.softwareSystems[Payment System]`); `docs/diff-schema.md` (additive-only policy, R-2); expect-test snapshots — **DONE 2026-11-21** (`Diff::to_json` on `preserve_order` maps ⇒ stable key order; all four categories always present, absent ⇒ `null`; snapshot pins one entry per category incl. a consistent-id `idChanged`) | M0b |
| **D-3** | Pipeline + CLI: `Request::Diff`, `ExitClass::Differences` (= 1, the documented P4-D1 extension), human format colored ± grouped by category (`ApiOptions` gains `color` by addition — CLI fills `!no_color && NO_COLOR unset`, defaults PLAIN so API/e2e bytes are stable); `oxidizr diff A B [--format human|json] [--ignore-layout] [--quiet]` (`--quiet` = exit code only), loads via `api::open` BOTH sides — **DONE 2026-11-21** (diff_cmd body in the pipeline; CLI smoke also caught the PROVENANCE-STAMP trap: `structurizr.dsl*` properties are loader-regenerated per parse and are now NEVER compared — the `diff(a,a)` property only holds with the exclusion; pinned by a unit test + CLI goldens + `diff-schema.md`) | M0c |
| **D-4** | Fixtures + close: vendored hand-curated pairs (`*.dsl`, `*.json` pairs) under `testdata/probes/diff/*` (README contract row), json snapshots on the pairs, full e2e tier + bless 307×unchanged + PROGRESS/RESUME — **DONE 2026-11-21** (the cross-format pair caught TRAP 2: loaders represent the SAME model differently — derived vs re-stored kind-default tags, `""` vs absent text — so the engine compares the reference EFFECTIVE views: `Tags::effective` (`getTags()`), empty text reads ABSENT; `diff(a, a)` two-load determinism sweeps over `testdata/dsl` + the legal JSON corpus (`testdata/validation/*` deliberately EXCLUDED — invalid fixtures self-pair non-empty by design); `tests/diff_probes.rs` 6 green, bless 307×unchanged) | M0d |
Locked-in consequences (decided here, documented in `diff-schema.md`):
a relationship DESCRIPTION or any element NAME change IS an identity
change (add+remove — the keys embed them); P4-V1 may refine the
`idChanged` taxonomy BEFORE the schema is published (it is v0-only until
the M0d stamp).

## §P4-M1 — `inspect` decomposition (CLOSED 2026-11-21; items I-1…I-4, commits M1a…M1d)

New crate `oxidizr-inspect` (deps: `oxidizr-core` only — the engine is pure
model logic; format+pipeline live in `oxidizr-api` per the T0-d invariant,
the CLI stays the argv shim). Ports `structurizr-inspection` @ `ad34036`:
`DefaultInspector` (run ORDER = the report contract — violations keep
execution order, no sort), `PropertyBasedSeverityStrategy`, `Violation`
(`type | severity | message` = the reference `toString`). **P4-V2 CLOSED at
decomposition** (read from the pinned Java, corrections to P4-D7 baked in):

- Property prefix is `structurizr.inspection.` (SINGULAR; the P4-D7 text
  guessed `structurizr.inspections.<InspectionName>` — WRONG), keys are the
  lowercased dotted TYPE path (`structurizr.inspection.model.component.description`)
  with wildcard fallbacks generated longest-first (`…model.component.*` →
  `…model.*` → `structurizr.inspection.*`); relationship types rewrite
  `.relationship.` → `.relationship[<srcType>-><dstType>].` (lowercase Java
  simple names: `softwaresystem`, `deploymentnode`, …), the SPECIFIC key tried
  first, and for relationship-scoped types the generic `…model.*` / `*`
  fallbacks are NOT tried (Java `subList(0, n-2)` quirk — ported).
- Holders searched inner→outer, first key hit wins: element ⇒ element →
  parent → grandparent → model → workspace; relationship ⇒ relationship →
  linked → source → source.parent → model → workspace; view ⇒ view-config →
  workspace; element-style ⇒ style → view-config → workspace; model-level ⇒
  model → workspace. Values `error|warning|info|ignore` (case-insensitive);
  NO match ⇒ ERROR — every inspection DEFAULTS to error, there are no
  per-inspection Java defaults (P4-D7 "mirrored from Java defaults" = all
  ERROR + this strategy). Unparseable values throw in Java; we map them to
  ERROR (errors-as-values rule, `agents/notes/process/faithful-port.md`).
- `structurizr.inspection` = `"false"` (any case) skips the whole run
  (`DefaultInspector` ctor); the four `structurizr.inspection.{error,warning,
  info,ignore}` counters are harness artifacts written onto the workspace —
  NOT computed here (the oracle strip already documents them).
- Type catalog (24 distinct `getType()` strings; three classes share
  `workspace.scope`, both documentation ones share `documentation.embeddedview`):
  `workspace.tooling`, `workspace.scope`, `model.empty`,
  `model.{person,softwaresystem,container,component,deploymentnode,infrastructurenode}.description`,
  `model.{container,component,deploymentnode,infrastructurenode}.technology`,
  `model.softwaresystem.{documentation,decisions}`, `model.deploymentnode.empty`,
  `model.element.disconnected`, `model.element.noview`,
  `model.relationship.{description,technology}`, `views.empty`,
  `views.view.{key,empty,layout}`, `views.styles.element.metadata`,
  `documentation.embeddedview`.
- Iteration order (faithful): elements = `Model.getElements()` TreeSet ⇒
  NUMERIC-id sort (`ModelItem.compareTo`); each element's OUTGOING
  relationships in numeric-id order (source-side ownership ⇒ each
  relationship inspected exactly once); per-kind loops run in
  `DefaultInspector` order; views iterate per kind in declaration order.
- Plan-doc correction: the "proptest `inspect(validate-clean) == []`" is
  unsatisfiable as written (missing descriptions are VALID but ERROR-flagged)
  — replaced by a **complete-workspace generator** proptest (descriptions +
  technologies + scope set + every element on a named view ⇒ `inspect == []`).

| Item | Deliverable | C |
|---|---|---|
| **I-1** | Crate skeleton + severity machinery: `oxidizr-inspect` (`lib`, `Severity`, `Violation {type, severity, message}`, `Inspection` type consts, `PropertyBasedSeverityStrategy` port incl. the wildcard chain, holder order, relationship rewrite + `subList` quirk, `"false"` kill-switch, value parsing ⇒ ERROR fallback); unit tests for every chain leg + holder precedence + relationship specificity — **DONE 2026-11-21** (11 unit tests; deviation note: Java THROWS on unparseable severity values, we map to ERROR — errors-as-values, fail-loud for CI) | M1a |
| **I-2** | Model-side inspections (workspace tooling/scope, `model.*` incl. disconnected/noview/empty-model/empty-deployment-node/multiple-systems, relationship description/technology, embedded-view missing/generated-key for workspace + softwaresystem/container/component documentables): exact Java message strings, `terminologyFor` (view-config terminology override, lowercased) + `nameOf` (canonical minus `Type://`) helpers; per-inspection trigger snapshots (`expect!`) on builder-built fixtures — **DONE 2026-11-21** (`run.rs` engine + 15 lib + 12 integration tests; blessed run-order snapshot audited against the Java: canonical names with the `.`/`/` separators, numeric-id `getElements()` merge, disconnected-exempts-deployment-nodes, noview uses `getName()`, custom-elements-only model IS "empty"; embedded-key matchers hand-written from the Java regexes under `Matcher.matches()` semantics) | M1b |
| **I-3** | View-side inspections (`views.empty`, per-view key/empty/manual-layout loops per kind, multi-system context/container-view scope checks, element-style metadata) + `inspect(workspace)` full run in `DefaultInspector` order; run-order integration snapshot; complete-workspace proptest (`inspect == []`) + corpus sweep (`testdata/dsl` never panics) — **DONE 2026-11-21** (view phase in `run.rs` verified verbatim against `DefaultInspector` — each model-view kind runs the key/empty/layout trio, the two multi-system checks count DISTINCT `softwareSystemId`s at their exact loop positions, filtered/image get the generated-key inspection ONLY, styles fire on `metadata == Some(false)`; `inspect_views.rs` run-order snapshot + holder-chain severity tests (view→view-config→workspace, style→view-config→workspace, views→view-config→workspace, inner-first exact-match wins, `*` wildcard fallback); `inspect_complete.rs` complete-workspace proptest; `inspect_corpus.rs` sweep over `testdata/dsl` — lossy-read for the Latin-1 fixture, unparsable fixtures skipped, engine total on every parsed workspace) | M1c |
| **I-4** | Pipeline + CLI: `Request::Inspect`, `inspect_cmd` via `api::open`; `oxidizr inspect <WORKSPACE> [--severity-threshold info\|warning\|error] [--only T]… [--except T]… [--format human\|json] [--quiet]`; human = `type \| SEVERITY \| message` lines (renderer-colored, PLAIN goldens), json = `{schema:1, violations:[{type,severity,message}]}` in run order; IGNORE never reported, threshold filters (rank `error>warning>info`), `--only` applied before `--except` (exact type match); exit 1 iff any REPORTED violation (`ExitClass::Differences` generalized to findings — same code 1, P4-D1 extension unchanged); CLI goldens; bless + PROGRESS/RESUME close — **DONE 2026-11-21** (`docs/inspect-schema.md` written next to the diff schema; `Severity` re-exported from `oxidizr-api`; 8 CLI goldens incl. the `open` default-views leg REPORTING the generated `SystemLandscape-001`/`SystemContext-001` keys — the pinned `createDefaultViews` quirk, and the element-property severity downgrade through `--severity-threshold`) | M1d |

## §P4-M2 — importers decomposition (**CLOSED 2026-11-21** — items N-1…N-6 all DONE, commits M2a…M2f; milestone log `PROGRESS.md`; started 2026-11-21)

Three arcs in one milestone (P4-D8 diagram import + P4-D10-adjacent WSD
exporter + P4-D13 `!docs`/`!adrs`) ⇒ charter decomposition up front.
**P4-V3 CLOSED at decomposition** (2026-11-21, read from the pinned
monorepo `ad34036` — `structurizr-import` + `structurizr-dsl` +
`ImageUtils` + `structurizr-export`), and it REVISES P4-D8:

- The reference `structurizr-import` diagram "importers" are NOT model
  parsers: PlantUML/Mermaid importers deflate/base64 the diagram text
  into a render-SERVER URL written to an `ImageView` (kroki-shaped; the
  fetch is network ⇒ stays deferred exactly as the plan's kroki clause
  already decided); `ImageImporter` wraps a LOCAL file → `ImageView`
  (data-URI content + guessed content type + `title = file.getName()`).
- NO C4-PlantUML/Mermaid→model parser exists ANYWHERE in the monorepo —
  the P4-D8 "reference-importer scope as the ceiling" (R-1) has no
  ceiling. Signed off 2026-11-21 (questions tool) as **F12** in
  `ADDITIONAL_FEATURES.md`: the macro importers ship as an agreed
  additional feature — scope = the P4-D8 macro list + vendored real-world
  samples, warning-and-skip, SEMANTIC fidelity bar (element/relationship
  sets match the source), output must load cleanly in the reference
  (oracle read gate) and stays pure Structurizr JSON.
- `ImageUtils` port specifics (the `--format image` leg): content type is
  JVM `guessContentTypeFromName` (`.png`→`image/png`, `.jpg/.jpeg`→
  `image/jpeg`, `.gif`→`image/gif`, `.svg`→`image/svg+xml`; anything not
  `image/*` ⇒ "is not a supported image file" error); data URI =
  `data:<ct>;base64,` + base64 — SVG as RAW bytes, but non-SVG passes
  through an AWT `ImageIO` READ+WRITE RE-ENCODINE (JVM-version-dependent
  bytes!). Port deviation (register in DEVIATIONS.md): ALL formats embed
  the raw file bytes — deterministic, same shape, and for SVG exactly
  what the reference does.
- `!docs`/`!adrs` (P4-D13-1, retiring OXZ-5003): `DocsParser` grammar
  `!docs <path> [fqn]` + `exclude <filename|regex>…`; paths resolve
  RELATIVE TO THE DSL FILE; the JVM picks importers by FQN — the Rust port
  keeps the same dispatch (registry of the six built-in FQNs + the
  `adrtools`/`madr`/`log4brains` aliases, unknown ⇒ reference-shaped "was not
  found"; NO per-directory auto-detection, `!adrs` DEFAULTS to adrtools; NO
  structurizr-native decision importer exists @ `ad34036`). P4-V7
  (status/date parsing) and P4-V8 (exclude matching: FULL-MATCH REGEX ⇒ no
  `globset`, std-only `regexlite`) CLOSED 2026-11-21 — full port contract in
  `IMPORT-DOCS-ADRS.md`.

Items (each ends green; commits M2a…M2f):

| # | Deliverable | Commit |
| --- | --- | --- |
| **N-1** | `oxidizr-import` crate skeleton (deps: `oxidizr-core` + `oxidizr-diag`, errors as values) + the **image importer** port (`ImageView` content/content-type/title semantics above, raw-bytes data URIs, `base64` workspace dep) + `data:` URI helper reuse check against the D10 embed machinery — **DONE 2026-11-21** (crate deps landed as core + base64 + thiserror — diag codes are not needed at engine level; `content_type_of` = the `ImageUtils` extension table; `import_image` rides `WorkspaceBuilder::create_image_view` (generated `Image-001`, order, uniqueness) + `view_base_mut` title; raw-bytes deviation registered as D14; D10 embed machinery checked — the viewer's data-URI stack is GUI-side, no core helper to reuse; 4 unit tests) | M2a |
| **N-2** | **C4-PlantUML model importer** (F12): line-oriented macro parser (`Person(_Ext)?`, `Person_Boundary`, `System(_Ext|_Boundary|_Backup|_Db|_Queue|_LandingPage)?`, `Container(_Ext)?`, `Container_Boundary`, `Component`, `Rel(_[A-Za-z]+)?` incl. the `System_Rel`-style prefixes as tags, `System_Ext_P`-style layout hints tolerated, `!include`/`!$`/`!$`-style preprocessor + `LAYOUT_*`/`SHOW_*`/`skinparam`/`title`/block devices tolerated), boundary nesting ⇒ grouped elements, tags from macro args, aliases (`$alias`) resolution; ~~model + ONE view~~ **MODEL ONLY — plan correction, 2026-11-21**: the reference `CustomView.checkElementCanBeAdded` (`CustomView.java` @ `ad34036`) permits ONLY `CustomElement`s and every scoped-view guard rejects a mixed C4 set, so NO view kind may host the declared subset — imported workspaces get native default views at open (`api::open` ⇒ `createDefaultViews`), the F12 bar is the semantic model; unknown macro ⇒ warning + skip (never fail-all); semantic-set assertions + `expect!` goldens — **DONE 2026-11-21** (`src/macrocall.rs` = shared plumbing: quote-aware comment stripping, `!$var`/procedure-block preprocessor, logical-line join, kwargs + `~"…~"` escapes; `src/c4plantuml.rs` = macro table pinned from plantuml-stdlib/C4-PlantUML (`Person(System)?(alias,label,descr,sprite,tags,link,type)`, `Container(Component)?(alias,label,techn,descr,…)`, `Boundary(alias,label,tags,link,descr)`, `Rel(from,to,label,techn,descr,sprite,tags,link)` — the label maps to the model DESCRIPTION, round-trip symmetry with `C4PlantUMLExporter`; `_Ext`⇒`External`/`Db`⇒`Database`/`Queue`⇒`Queue` tag spellings, `Rel_Back` REVERSES, `BiRel` mints BOTH directions, direction suffixes are layout hints, legacy `System_Db`/`SystemRel` spellings accepted; boundary scope stack incl. next-line `{`; chrome tolerated, raw arrows warn); 16 tests, blessed digest golden) | M2b |
| **N-3** | **Mermaid C4 model importer** (F12): `C4Context|C4Container|C4Component|C4Deployment` bodies — `Person/Person_Ext/System/System_Ext/Container/Container_Ext/Component/Component_Ext(alias, "name", "desc" [, "tags"])`, `System_Boundary/Container_Boundary/Container_Boundary_Ext(alias, "name") { … }`, `Rel(_[A-Za-z]+)?(alias|src, alias|dst, "label" [, "tech" [, "tags"]])`, `BiRel`, `UpdateElementTag/UpdateLayoutConfig/UpdateRelStyle/SetLayoutTag/Enter_Boundary/Leave_Boundary/SHOW_*/`title tolerated; `-->` plain-flowchart edges NOT promised; same warning-and-skip + fixture bar — **DONE 2026-11-21** (the C4 machinery is dialect-parameterized instead of duplicated: `Dialect{PlantUml,Mermaid}` selects the positional slots — `Mermaid` spells `Person/System(alias,label,descr,?tags)`/`Container(Component)?(alias,label,techn,descr,?tags)`/`Rel(from,to,label,techn,?tags)` with NO positional `$descr` (keyword-only via `usize::MAX`); `%%` comments via `strip_comments_mermaid`; `Container_Boundary_Ext` = container scope + `External`; `TOLERATED` + `Enter_Boundary`/`Leave_Boundary`/`Set*`/`Show*`/`Hide*`/`CLEAR_*`; the skip-warning noun comes from `dialect.name()`; the golden digest moved to `src/testutil.rs` and is shared; `src/mermaid.rs` = thin wrapper + 7 tests incl. a blessed digest golden over a `C4Context` with `%%` chrome, positional + `$tags=`/`$descr=` kwargs, `SystemQueue`/`ContainerDb` shape tags, `Rel_Back`/`BiRel`) | M2c |
| **N-4** | CLI arc: `Request::Import { input, format, view_key, view_name, output }` + `import_cmd` (fresh workspace per diagram; format by `--format` else extension sniff `.puml/.iuml/.c4p`→c4plantuml, `.mmd/.mermaid`→mermaid, image extensions→image; warnings ride `Outcome::log`, workspace bytes via the standard writer — format by output extension, like `dsl2json`); `oxidizr import <FILE> [--format …] [--view-key K --view-name N] [-o OUT]` ships COMPLETE with all three formats (no half-shipped surface); CLI goldens per format + the read-check (imported JSON re-opens clean through `api::open`) — **DONE 2026-11-21** (`ImportFormat` lives in `pipeline.rs` + re-exported from `oxidizr-api`; `import_cmd`: explicit `--format` wins, else `sniff_format` on the file name (`.puml/.iuml/.c4p`→c4plantuml, `.mmd/.mermaid`→mermaid, the `ImageUtils` extensions→image), nameless input or image-from-stdin ⇒ `ExitClass::Usage` + OXZ-5006; skip notes ride `LogItem::NoteLine`; bytes via `oxidizr_json::to_vec` + `emit_bytes` like `dsl2json`; CLI clap enum `ImportCmdFormat` spells `c4plantuml`; 9 goldens in `tests/cli.rs` §P4-M2 incl. the re-open read-check asserting `createDefaultViews` fires) | M2d |
| **N-5** | **WSD exporter** (reference port: `websequencediagrams/WebSequenceDiagramsExporter`, 171 LOC, dynamic views only) → `oxidizr export --format websequencediagrams`; P2 exporter-family ritual (Diagram value, goldens, semantic comparator where the oracle speaks WSD) — **DONE 2026-11-21** (`oxidizr-export/src/wsd.rs` = `WsdDialect` on the shared `Engine` (all boundary hooks empty; `render_as_sequence_diagram`/`owns_sequence_document` true ⇒ the dialect owns the dynamic walk via `sequence_document`) + `find_terminology` (the `Terminology.findTerminology` port: instance unwrap, per-kind override-or-default, `CustomElement` ⇒ metadata-else-`"Element"`, config ignored) + `export_all` (exports ONLY `View::Custom` + `View::Dynamic` — the static/deployment overloads return `null`; custom views ride the engine base-body walk: view-order elements, id-sorted rels, guarded blank; dynamic: header `title <name>` (+ real-newline description), participants from the relationship endpoints source-then-destination first-seen (`actor` for a `Person`, else `participant`, literal `\\n` before `<name> as <name>`), UNCONDITIONAL blank after participants, stored-order relationships (`->`/`->>`, response swaps + `-->`/`-->>`; view override trims-empty to the model description, both absent ⇒ EMPTY after `": "` — oracle witness), no frames/legend, ext `wsd`, key = view key; 8 unit tests incl. the `custom-view-animation` byte witness; oracle 9/9 fixtures byte-parity (scratch sweep) + `golden/websequencediagrams/` 11 files/9 fixtures via `cargo xtask oracle gen --wsd` (new gen mode + verify leg) + `export_wsd_goldens.rs` Category A; format dispatch arm + arm test, the unlanded test now `static`-only; OXZ-5007 allow-listed PERMANENT in the coverage gate (only `static` raises it, the export matrix runs the four diagram formats); 161 e2e sidecars re-blessed: the wsd slots flip from the 5007 probe to observed values) | M2e |
| **N-6** | **`!docs`/`!adrs` importers** (P4-D13-1): ports of `DefaultDocumentationImporter`/`FormatFinder` (+ recursive walk) and the decision family (`AbstractDecisionImporter` id/status/date parsing, `AdrTools`/`Madr`/`Log4brains`, structurizr-native `.md`+`.json`? — inventory per P4-V7 FIRST); the DSL statements already parse (P1) — wire real import at the OXZ-5003 site, retire the diagnostic + the deferred-corpus exclusion in `testdata/README.md`; P4-V8 decides exclude matching (Java regex vs glob ⇒ `globset` returns only if globs); vendored ADR corpora + goldens — **DONE 2026-11-21** (ports `docs.rs`/`decisions.rs` + std-only `regexlite` for the P4-V8 full-match exclude; engine `run_import` per the pinned `end()` order — skip on string parses, `loadClass` first, empty-exclude guard, reluctant image pass; OXZ-5003 retired to a tombstone, m4 register 69 clean / 35 registered, e2e bless diff = exactly the two intended sidecars; oracle A/B green — documentation byte-parity on both positive fixtures, negative legs match the JVM error text verbatim; A/B caught + fixed the Java empty-input `split` special case ⇒ `"Element,Deployment Node,"`; contract `IMPORT-DOCS-ADRS.md`, deviations D15/D16) | M2f |
| **Close** | oracle read gate green (Category B, `$OXIDZR_STRUCTURIZR_BIN`), fixtures vendored with attribution (`testdata/import/*` + README contract row), bless 307×unchanged, full e2e tier, PROGRESS/RESUME stamps — **CLOSED 2026-11-21** (commit M2f): A/B positive legs byte-parity on `documentation` + masked-workspace parity, negative legs verbatim; the corpora were already vendored byte-exact under `testdata/dsl/{docs,decisions}/` (README rows) so no `testdata/import/*` tree was needed; bless = 305×unchanged + the 2 reviewed OXZ-5003-retirement sidecars; full e2e tier + `cargo xtask ci` green | M2f |

## §P4-M3 — `query`/`init`/`completions`/`generate` decomposition (started 2026-11-21; items O-1…O-4, commits M3a…M3d)

**Plan correction (reality vs P4-D3, stamped before implementation):** the
`oxidizr_core::filter::Predicate` type P4-D3 assumed DOES NOT EXIST —
`core::filter` is the P3-D3 tag-based FILTERED-VIEW evaluator (mode + tag
list, webapp semantics), not an expression type. The query language is the
ITEM expression grammar (`ExpressionParser` @ `ad34036`), already ported
and pinned in `oxidizr-dsl/src/engine/expr.rs` (`ExprScope::{Model,Static,
Deployment,Custom}`). The fix reuses THAT evaluator (one parser, one
evaluator — the plan's intent) instead of duplicating the grammar: a thin
public `oxidizr_dsl::query` module feeds an adopted workspace into the
engine's `ExprScope::Model` evaluation. P4-V4 (grammar inventory — what
`query` can promise) closes with O-1's first commit.

`generate` signature witness (reference CLI 2026.10.26, app code not in
the vendored monorepo, help output pinned 2026-11-21):
`generate [-exclude R] -i <path> [-include R] -o <path> [-relationships first|all]`
— ONE output workspace from (potentially) MULTIPLE input workspaces
(`SystemLandscapeGenerator.generate(Collection<Workspace>)` @
`structurizr-core/util` — vendored and read; it creates NO views and sets
name `System Landscape` + scope `Landscape`; whether the app subcommand
adds views after the generator is an O-4 oracle-witness question). Our
surface keeps P4-D9's spelling (`oxidizr generate system-landscape`,
`--format dsl|json`, `-o`) widened to 1..N positional workspaces +
`--include`/`--exclude`/`--relationships` to match the reference.

Items (each ends green; commits M3a…M3d):

| Item | Scope | Commit |
|---|---|---|
| **O-1** | `query` (P4-D3): close P4-V4 (grammar token inventory from the pinned `ExpressionParser`/`ExpressionBuilder` + our `expr.rs` port); public `oxidizr_dsl::query::run(&Workspace, expr) -> QueryResult` (engine reuse: `WorkspaceBuilder::adopt` + the `structurizr.dsl.identifier` register path from the JSON-extends leg + `ExprScope::Model`); `Request::Query` + `oxidizr query <WS> --filter EXPR [--kind element\|relationship\|view\|all] [--format table\|json\|ids] [-o]` — table `id kind canonicalName [tags…]`, ids one-per-line, json = schema-1 doc (next to the diff/inspect schemas); views by the documented key/name/tag predicate subset (V4 cataloged); tests: evaluator matrix (P3-V5 corners), CLI goldens on vendored workspaces, proptest: `--kind element --format ids` output identical from the `.dsl` and its JSON twin — **DONE 2026-11-21** (`oxidizr-dsl/src/query.rs` public entry — engine reuse through `adopt` + the extracted `register_json_identifiers` shared with the JSON-extends leg; 5 unit tests pin the grammar spellings: BARE values, enum-spelling type tokens, Model-scope fail-soft EMPTY, `&&`/`||`/`->` arms; API report layer `oxidizr-api/src/query.rs` — table/ids/schema-1 formats, EFFECTIVE tags + canonical names via the `diff` D-4 machinery, view subset mirrors `parseExpression` EXACTLY (` && ` first, halves are single arms — a compound half matches nothing, `[0]`/`[1]` drops a third arm; 5 unit tests); `query_cmd` in `pipeline.rs` (zero matches exits 0, throwing arm ⇒ Diagnostics, no new codes); CLI `--kind`/`--format`/`-o` + 4 goldens; twin witnesses + never-panics corpus sweep `query_probes.rs` 6 tests — the twin in the exact `--kind element --format ids` form; `docs/query-schema.md` = schema 1 + the P4-V4 inventory, CLOSED above. Plan corrections on the row: views have NO name/tags — the subset is key/TITLE/description; the JSON corpora are `golden/` + `read-corpus/`.) | M3a |
| **O-2** | `init` (P4-D4): `oxidizr init [DIR] [--template minimal\|c4] [--force]` — `include_str!` templates (`workspace.dsl` + README + `.gitignore` + `.editorconfig` + `.vscode/settings.json` with the lsp note), zero network zero deps; idempotent (existing files skipped with a note, `--force` overwrites); prints the tree + next commands; the c4 template = the canonical 3-system starter; CI teeth: template output passes `validate` + a `dsl2json` golden (drift breaks CI — intended) — **DONE 2026-11-21** (templates `crates/oxidizr-api/templates/{minimal,c4}/workspace.dsl` + `templates/shared/*`; stored CANONICAL — they pass `fmt --check`, so the printer's conservative unquoting (`person User`) is the required spelling; grammar fact pinned the first draft: view headers take `[identifier] [key] [description]` ONLY — titles via in-view `title`/`description` statements. `oxidizr-api/src/init.rs` = pure template table (`files()` in REPORT order, 2 unit tests incl. zero-error parse teeth); `Request::Init` + `init_cmd` in `pipeline.rs` (report bytes `created <rel>` / `skipped <rel> (exists, use --force to overwrite)` + `next steps:` footer, exit 0 even all-skipped, Io failure = exit 3 with first `Issue::io`, nested dirs created); CLI `Command::Init` + `InitCmdTemplate` (default `c4` — kebab value spellings: `C4`, not `c-4`); tests: `init_probes.rs` 4 (tree + idempotence + force + nested, and the P4-D4 teeth: scaffold validates + masked `dsl2json` expect! golden per template — mask strips `structurizr.*` property keys, the loader-regenerated-UUID class) + 2 CLI goldens pinning the report bytes) | M3b |
| **O-3** | `completions` (P4-D10): `oxidizr completions <bash\|fish\|zsh\|powershell\|elvish>` via `clap_complete` (the one adopted new dep), per-shell goldens + `bash -n`/`zsh -n` smoke where the shell exists — **DONE 2026-11-21** (`clap_complete` 4.6 added to the workspace table = the ONLY new dep, clap-lineage sync; `Command::Completions { shell: clap_complete::Shell }` dispatched in `run` BEFORE the pipeline like `view` — it renders the argv table itself, never a `Request`; script bytes go straight to stdout (payload, like `dsl2json`), exit 0; unknown shell = clap's 2. Tests: `surface_parses` arms + 2 CLI tests — every shell renders with a stable entry/registration line pinned (bash `_oxidizr() {` + `complete -F _oxidizr … oxidizr`, zsh `#compdef oxidizr`, fish `complete -c oxidizr`, powershell `Register-ArgumentCompleter -Native -CommandName 'oxidizr'`, elvish `set edit:completion:arg-completer[oxidizr]`) + a real subcommand token in every script, and the `bash -n`/`zsh -n` smokes run where the binary exists (both green here)) | M3c |
| **O-4** | `generate system-landscape` (P4-D9): `SystemLandscapeGenerator` port (name-first person/system clone, description-elsewhere-wins, group first-wins, url/tags/properties/perspectives clone; relationship re-derivation `interactsWith`/`uses`/`delivers` guarded by `hasEfferentRelationshipWith` per First/All; `{workspace:<id>}/diagrams` hyperlink onto the first CONTAINER-BEARING system; "Person or software system with name … not found" warning path); 1..N workspaces + `--include`/`--exclude` regex filters + `--relationships first\|all`; app-layer ONLY (Locked list: no view generation in parser/library — whether the reference app adds views post-generator is an oracle witness BEFORE we decide); deterministic ⇒ Category A goldens + a Category B oracle leg via `$OXIDZR_STRUCTURIZR_BIN generate system-landscape` — **DONE 2026-11-21** (the oracle witness settled the row's open question: the reference adds NO views — the port matches. `oxidizr-api/src/generate.rs` = the pure `system_landscape` merge (name-first person/system clone; description AND url set UNCONDITIONALLY per workspace so the LAST wins — nulls nuke, witnessed; group first-wins; stored-tags union via `Tags::add`; properties/perspectives merge; person/system↔person/system relationship re-derivation per First/All guarded by the `hasEfferentRelationshipWith` twin (First = any efferent to the destination, All = same description); one shared id counter, elements then relationships; `{workspace:<id>}/diagrams` onto the FIRST container-bearing system per workspace (file-loaded id 0); `findElement` searches people first — a same-named person shadows a system (ported quirk); the "Person or software system with name … not found." warning path ported — dead code in the reference, returned as warnings) + the `pathfilter` std-only full-match engine (the reference applies `Pattern.matches` — an IMPLICIT FULL match over the FULL path: literals, `.`, `\d \D \w \W \s \S`, `[...]` classes with ranges/`^`-negation (shorthands only as escapes: `[d]` is the letter), `(...)`/`(?:...)`, `|`, `* + ?` — the lazy spellings parse and share the node, a boolean full match is greediness-independent —, `^`/`$` accepted; `{m,n}`/lookaround/`\p{…}`/named groups/inline flags ⇒ the new OXZ-5010 `GENERATE`, never a silent widening; 6 unit tests incl. the witnessed probe matrix). `Request::Generate` + `generate_cmd` in `pipeline.rs` (filters compile FIRST ⇒ OXZ-5010 exit 1 before any file is touched; sorted recursive scans with include/exclude over `to_string_lossy`; an explicit file positional ALWAYS loads — documented deviation, the reference applied its include to a single `-i` file too; a per-file load failure is the reference `log.warn` skip (note); ZERO loadable workspaces = the ported `Two or more workspaces must be provided`, exit 1 — ONE workspace is legal despite the wording, witnessed). CLI `generate system-landscape <1..N paths> [--include R] [--exclude R] [--relationships first|all] [-o out]` — output ALWAYS JSON regardless of the `-o` name; `command_name` = "generate". Tests: `generate_probes.rs` 6 (witnessed merge `expect!` goldens under BOTH strategies, `structurizr.*`-masked; filter legs; single-workspace-legal; OXZ-5010; missing-path exit 3; skip-note legs) + 1 CLI golden (every exit class through the binary) + the Category B leg `cargo xtask oracle landscape` (reference `-i dir -o out.json` TZ=UTC vs in-process `execute`, semantic facts with endpoint ids resolved to names — 10 facts match).) | M3d |
| **Close** | `cargo xtask ci` green, bless (expect unchanged — new commands never touch the pipeline outputs the corpus pins), full e2e tier, PROGRESS/RESUME stamps | M3d |

## P4-D12 Verification list

- P4-V1 diff identity rules vs real-world edit sequences (identifier/canonical-name
  stability limits) — refine `idChanged` taxonomy before freezing `schema: 1`.
- P4-V2 exact inspection severity property keys + Java default severities —
  **CLOSED 2026-11-21** (read from `structurizr-inspection` @ `ad34036`: keys are
  `structurizr.inspection.<lowercased dotted type path>` with `*` wildcard
  fallbacks + the `[srcType->dstType]` relationship rewrite; ALL inspections
  default to ERROR, properties override — full findings + port plan in the
  §P4-M1 decomposition above; P4-D7's `structurizr.inspections.<Name>` guess
  was wrong).
- P4-V3 macro/syntax inventory: reference plantuml/mermaid importer accepted forms vs
  vendored real-world samples (drives importer scope) — **CLOSED 2026-11-21**
  (pinned monorepo read: NO model-parser importers exist in the reference —
  `structurizr-import` diagrams = ImageView server-URL/data-URI wrappers;
  macro importers = F12 additional feature; full findings in §P4-M2).
- P4-V4 filter/expression grammar full token inventory (what `query` can promise)
  from `ExpressionBuilder`-class sources — **CLOSED 2026-11-21** (pinned
  `StructurizrDslExpressions` @ `ad34036` read against our `expr.rs` port:
  17 tokens, 1:1, port complete; no `ExpressionBuilder` class exists — the
  arms live in `ExpressionParser`/its view subclasses. No `!=` forms for
  `type`/`group`/`properties`; bare `->` throws; ` && ` checked before
  ` || `, Java `split` + `[0]`/`[1]` = exactly TWO arms. Full table +
  value-case rules: `docs/query-schema.md` §The query language.)
- P4-V5 `lsp-server` version + stdio framing behavior under fast edits.
- P4-V6 cargo-dist musl/aarch64 feasibility with the pinned nightly; signing deferred
  (unsigned v1 — documented, not deviating from anything).
- P4-V7 (ex-P1-V9) ADR status/date parsing corners per tool format (MADR v2/v3,
  log4brains front matter) — before the importer code (P4-D13). — **CLOSED
  2026-11-21** (pinned monorepo read; per-tool parsing contract + machine-TZ
  date deviation (D15) in `IMPORT-DOCS-ADRS.md`; no structurizr-native decision
  importer exists — the "4th format" is a non-goal).
- P4-V8 (ex-P1-V6) `!docs`/`!adrs` exclude-pattern semantics (glob vs path prefix) —
  verify with reference before trusting globset config (P4-D13). — **CLOSED
  2026-11-21** (pinned: `name.equals(x) || name.matches(x)` = Java regex FULL
  MATCH, NOT glob ⇒ `globset` does NOT return; std-only `regexlite` matcher,
  subset + ported patterns in `IMPORT-DOCS-ADRS.md`).

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
