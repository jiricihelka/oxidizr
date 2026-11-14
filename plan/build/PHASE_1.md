# PHASE_1.md — detailed implementation plan: parsers & writers

Scope per PLAN.md Phase 1. This file records every architectural decision needed so
implementation can proceed without re-litigating design. Decisions are numbered
`P1-Dn`; compatibility contract, deviations (D1–D7) and Locked list in `DEVIATIONS.md`
override everything here.

Exit gate: every fixture in `testdata/` parses **except the `!docs`/`!adrs` corpus**
(2026-11-14 rebalance: doc/ADR importers moved to Phase 4 — D7; P1–P3 recognize those
directives with a `deferred to Phase 4` diagnostic, OXZ-5003); written JSON
byte-matches goldens from the pinned reference; read-corpus loads; `fmt` invariant +
`reconcile` tests green; `--strict` off by default.

---

## P1-D1 Crate topology (7 workspace members)

```
crates/
  oxidizr-core    domain model + validation; NO deps (not even serde)
  oxidizr-ast     DSL CST: node types, spans, trivia; no parser logic
  oxidizr-dsl     lexer, parser (CST builder), AST→workspace builder, formatter; deps: core, ast, diag
  oxidizr-json    JSON reader/writer (serde derives); deps: core, diag
  oxidizr-diag    Span, Severity, Code (OXZ-nnnn), Diagnostic, collectors; NO deps
  oxidizr-cli     binary: commands, flags, miette rendering, reconcile logic; deps: all above
  xtask           dev-only: oracle (golden gen/verify via $OXIDZR_STRUCTURIZR_BIN),
                  vendor, update-goldens
```

- Dependency graph: `ast ← dsl`; `core ← dsl, json`; `diag ← dsl, json, core(?no), cli`;
  `core` and `diag` stay dependency-free so everything can use them.
  `core` produces validation errors carrying `Option<Span>`; converting to `Diagnostic`
  happens in `dsl`/`json`/`cli` (core never imports diag).
- Rationale: LSP (P4) consumes `ast` + `diag` without pulling the parser; `fmt` lives in
  `dsl` (it needs trivia-preserving CST); `reconcile` lives in `cli` (pure composition
  of dsl+json, no reuse expected).
- Workspace settings: edition 2024, `rust-version` = current stable, `[workspace.lints]`
  with `clippy::all` + `pedantic` warn, `-D warnings` in CI; `unwrap()` only in tests.

## P1-D2 Errors & diagnostics

- Per-crate `thiserror` enums (`dsl::ParseError`, `json::JsonError`, `core::ValidationError`).
- `oxidizr-diag` owns: `Span { file: FileId, start: (line, col), end }` (1-based, UTF-8
  char counts), `Severity::{Error,Warning,Info}`, `Code` newtype with registry ranges:
  OXZ-1xxx lexer, 2xxx parser/grammar, 3xxx model & validation, 4xxx JSON, 5xxx I/O &
  CLI policy (e.g. OXZ-5001 network disabled, OXZ-5002 JVM-only directive, OXZ-5003
  directive deferred to Phase 4 — `!docs`/`!adrs`), 9xxx internal.
- Collectors: parser and validator return `Vec<Diagnostic>` (cap 100, then "too many
  errors" abort note). All codes stable, documented in `diag` rustdoc; no breaking code
  changes post-1.0 without DEVIATIONS.md entry.
- `cli` converts to `miette::Diagnostic` impls at the boundary (labels, snippets,
  help text, suggestions via Levenshtein over scope-permitted keywords + known
  identifiers); renders with `--pretty` (default when tty), plain fallback otherwise.

## P1-D3 DSL parsing pipeline (hand-rolled, no parser libs)

Four stages, each independently testable:

1. **Preprocess** (`dsl::preprocess`): BOM strip → split lines → handle `/* */` blocks,
   `"""` text blocks (become hidden constants referenced by placeholder), `\`
   continuations → `${VAR}` substitution pass (`!const`/`!var` map from a first-pass
   scan of the fully-included file set; env fallback only with `--env`, see P1-D8).
   Substitutions are recorded per line (pre→post mapping) so diagnostics quote the
   ORIGINAL line text (fixes a reference annoyance within D1 freedom).
2. **Lexer** (`dsl::lexer`): line-oriented tokenizer borrowing `&str` (zero-copy);
   quote-aware (`"…"` with `\"`), produces `Token { kind, text span, line }`; comment
   `//`/`#` and whitespace preserved as trivia for the CST (formatter needs them).
3. **CST builder** (`dsl::parser`): recursive descent driven by a **scope-context
   state machine** mirroring the reference: each context = enum with a
   `permitted_tokens()` set (verbatim from `plan/DSL.md` §10 + parser sources).
   Unknown token → error `OXZ-2001 "X is not valid here"` **plus** "did you mean" from
   the permitted set; recovery: skip to next statement line, continue (multi-error, D1).
   Every statement becomes a CST node in `oxidizr-ast` (kind-tagged, children + trivia +
   spans; lossless: `text()` reconstructs the source byte-for-byte — fmt invariant).
4. **Workspace builder** (`dsl::builder`): CST → `core` workspace via
   `core::WorkspaceBuilder`; two passes: (a) definitions/identifiers (flat|hier per
   `!identifiers`), (b) relationships + view contents (forward references allowed);
   then implied relationships, archetype application (pass 'a' honors archetypes),
   portability tracker → `structurizr.dsl`/`structurizr.dsl.identifier` properties
   (Locked rules), workspace metadata.

- Includes are resolved at parse entry (`dsl::loader`): file/dir (sorted path order,
  verify against reference — catalog item V-6); `workspace extends` merges base via
  `json`/`dsl` load then overlay (ids preserved from base per reference).
- JVM-only tokens (`!script`, `!plugin`, `!components`, `!impliedRelationships <fqcn>`):
  lex + recognize, emit OXZ-5002 with construct name + alternative advice; no partial
  build (D2).
- `!docs` / `!adrs` (2026-11-14 rebalance): P1 **lexes and parses the statement form**
  (valid args, spans — so P4 lands onto a finished grammar path) but the importers are
  deferred: loading such a workspace emits OXZ-5003 "documentation import arrives in
  Phase 4" and fails (no partial documentation). Interim behavior recorded as
  DEVIATIONS.md D7; P4 replaces the diagnostic with the real importers.

## P1-D4 Core domain model (`oxidizr-core`)

- Immutable after build: `WorkspaceBuilder` accumulates (checked ops), `build() ->
  Result<Workspace, Vec<Diagnostic>>`; `Workspace` = plain data, cheaply cloneable
  (`Arc<str>` names). No `Option` soup: kind-specific payloads in enums.
- Shape sketch (field names = JSON keys where possible, camelCase handled in `json`):

```rust
Workspace { id: Option<i64>, name: Option, description: Option, version: Option,
  last_modified: Option<ModifiedStamp>, thumbnail: Option, properties: BTreeMap,
  model: Model, views: ViewSet, documentation: Documentation, configuration: Configuration }
Model { people: Vec<Person>, software_systems: Vec<SoftwareSystem>,
  deployment_nodes: Vec<DeploymentNode>, custom_elements: Vec<CustomElement>,
  relationships: Vec<RelationshipId> /* stored in flat arena, referenced */,
  properties: BTreeMap, next_id: Cell<i64> /* sequential counter, Locked */ }
Element enum (Person|SoftwareSystem|Container|Component|CustomElement) +
DeploymentElement enum (DeploymentNode|InfrastructureNode|SoftwareSystemInstance|
ContainerInstance); shared `ModelItem { id: ElementId(String), name, description:
Option, tags: Tags, url: Option, properties: BTreeMap, perspectives: Vec, group: Option }`
Tags: insertion-ordered unique Vec<String>, defaults-first (Locked stacks in core consts)
Views: ViewSet { landscape/context/container/component/dynamic/deployment/filtered/
custom/image: Vec<…>, configuration }; ViewKind-specific scoped ids as newtypes
Ids: newtype String `ElementId`/`RelationshipId`, `ViewKey(String)`; env name `Environment(String)`
```

- Validation catalog lives in `core::validate` — one function per rule from the 16
  `workspaceValidation/*.json` fixtures + `WorkspaceScopeValidator` rules; each returns
  Diagnostics (no exceptions), runs on load and `validate` (P1-D8).
- Canonical names, default tags, implied-relationship strategy (only the default
  unless-any-exists strategy — matches D2 rejection of fqcn), id counter incl.
  `found(id)` bump-on-load: all in `core`, each unit-tested against `plan/FEATURES.md`
  tables.

## P1-D5 JSON I/O (`oxidizr-json`) — serde derives, streaming, no Value pass

- **Writer** = serde derives on model types + `#[serde(rename_all = "camelCase")]`
  where names diverge; **fields declared alphabetically** (that IS the key sort —
  serde_json streams struct fields in declaration order; its Map is a BTreeMap so
  `properties`/kept-unknown maps sort for free). `to_string_pretty` = 2-space + `\n`
  (matches contract).
- **Analysis (why no intermediate `Value`)**: Value pre-sort would add a full
  serialize→parse→sort→write cycle for every file while gaining nothing —
  alphabetical streaming output needs only alphabetical declarations; `--keep-unknown`
  is the sole case needing raw maps, and its output is intentionally non-parity
  (preserves what the reference drops), so `#[serde(flatten)] unknown: BTreeMap<String,
  Value>` appended after known fields is acceptable there. Byte-parity gate: goldens +
  CI invariant test (below).
- **Omissions**: `skip_serializing_if` per field: `is_none` for Options; `is_empty` for
  Strings/Vecs/BTreeMaps. Empty nested objects ARE written (`configuration {}`,
  `documentation {}` — golden-verified). Per-field quirks (e.g. `z == 0` omission on
  element views) are pinned by the **write catalog** (P1-D6) — do not guess.
- **Reader**: serde derives; leniency via small custom deserializers: `string_or_number`
  → String ids, `empty_as_none`, numeric tolerance, `enterpriseContextViews` alias via
  `#[serde(alias)]`, unknown ignored (serde default). Legacy/unknown capture only under
  `--keep-unknown` (D3): reader pass collects non-schema keys per object into the
  flatten map. Reader diagnostics: structural errors (wrong types) map to OXZ-4xxx with
  a JSON pointer path (`views.containerViews[3].automaticLayout.rankSeparation`).
- Dates: `chrono` with one custom ser/de pair: write `yyyy-MM-dd'T'HH:mm:ss'Z'` UTC
  (truncate subsecs); read accepts that exact form + offset + fractional-second
  variants.

## P1-D6 Write-order & omission catalog (V-list task)

Before writer code freezes: derive from reference goldens + sources the exact iteration
order of every JSON array (Java `TreeSet` comparators: people/systems/containers/
components/deploymentNodes/relationships-per-element/views/elementViews/
relationshipViews/animation steps/perspectives/healthChecks…) and per-field omission
quirks. Deliverable: `plan/build/WRITE-CATALOG.md` (table: JSON path → order rule →
omission rule → source evidence) + a `oxidizr-json` test module implementing each rule
explicitly (sort functions, not incidental order). Catalog items are the V-list §V-1.

## P1-D7 Formatter (`fmt`, in `oxidizr-dsl`)

- Printer over the trivia-preserving CST: canonical 4-space indent (matches reference
  docs examples; single source const), one statement per line, blank-line compaction
  (max 1), comment attachment preserved, string quoting normalized (quote only when
  needed by the tokenizer).
- Invariants (golden + property round-trip tests): (1) lossless CST ⇒ `parse(fmt(x))`
  builds the identical workspace as `parse(x)`; (2) `fmt(fmt(x)) == fmt(x)`; (3) the
  `structurizr.dsl` portability property of any JSON emitted from a formatted file uses
  the FORMATTED source (it is a fresh parse; no special case needed — document).

## P1-D8 CLI surface (`oxidizr-cli`)

- Commands (clap derive): `dsl2json [DSL] [-o out]`, `json2json [JSON] [-o]`,
  `validate [FILE]…` (auto-detect by extension, `WorkspaceUtils` parity), `fmt [DSL]…
  [-w|--write] [--check]`, `reconcile --dsl X --layout Y [-o]`.
- Global flags: `--strict` (warnings→errors), `--keep-unknown`, `--env` (enable env-var
  `${VAR}` fallback; default off — MUST verify reference feature default, V-3),
  `--no-color`, `--quiet`. `!include <url>` / `workspace extends <url>`: rejected
  OXZ-5001 (no network in P1 — D5; `--allow-net` lands in P2).
- `reconcile`: load layout JSON + re-parse edited DSL → map old→new ids via
  `structurizr.dsl.identifier` (exact match first; canonical-name fallback; ambiguous =
  OXZ-3xxx error entries, never guess) → rewrite view element/relationship ids → emit
  standard JSON + human report of remapped/dropped/ambiguous entries.
- Exit codes: 0 ok / 1 diagnostics errors (or `--strict` warnings) / 2 usage / 3 I/O.

## P1-D9 Dependencies (lean set, agreed)

Prod: `serde`+`serde_json`, `thiserror` (libs), `anyhow`+`clap`+`miette` (cli), `chrono`
(default-features off, `std`). (`globset` was here for docs/ADR excludes — moves with the
importers to P4.)
Dev: `expect-test`, `criterion` (benches + M0 baseline), `proptest`+`arbitrary`,
`cargo-fuzz` (nightly; lexer/parser targets, corpus = fixtures), `cargo-llvm-cov` (CI
report only). None of: indexmap (Vec+explicit sorts; maps are BTreeMap), chumsky/nom/
winnow/pest, miette-in-libs, serde_yaml (ADR front matter hand-parsed: status/date
only), rayon. Toolchain: pinned nightly in `rust-toolchain.toml` (date set at M0) with
the `NIGHTLY.md` feature register (AGENTS.md policy).

## P1-D10 Test & golden strategy

- `testdata/` layout: `dsl/` (vendored reference fixtures, Apache-2.0 attribution
  README + upstream commit), `golden/` (generated `.json`), `read-corpus/` (v5-era +
  back-compat workspaces), `validation/` (the 16 rule fixtures). The `!docs`/`!adrs`
  fixtures are vendored **but excluded from the P1 parse/golden gate** (deferred list
  in `testdata/README.md`; P4 retires the exclusion).
- Harness: `cargo xtask oracle gen` / `verify` invoke the official `structurizr`
  application **binary** via the AGENTS.md discovery rule (`$OXIDZR_STRUCTURIZR_BIN` →
  PATH; the nix dev flake pins it; version recorded in `testdata/README.md`). The tool
  is slow: goldens are **committed** and regenerated sparingly; oracle verification is
  the Category B job (`--features reference`, skip=fail there), never per-PR default.
  Pure-Rust `cargo test` never touches it (AGENTS.md §Testing).
- Per-module: `expect-test` snapshots for tokens/CST/parse errors; hand-rolled
  round-trip property for fmt; writer key-sorted CI invariant (scan output for object
  keys out of order); portability-matrix test (each trigger → property present/absent);
  diagnostics snapshot incl. suggestions.
- CI gates: fmt --check, clippy -D warnings, Category A tests, key-sort invariant — per
  PR; plus the scheduled/manual **reference job** (`cargo test --features reference` +
  `xtask oracle verify`, skip = failure).

## P1-D11 Milestones (ordered, each ends green)

- **M0** scaffold: workspace, 7 crates, `rust-toolchain.toml` + `NIGHTLY.md`, `flake.nix`
  (pinned nightly + JDK + structurizr bin for Category B; plain `cargo` still works
  without nix), MIT OR Apache-2.0 LICENSE files + workspace metadata,
  `docs/diagnostics.md` stub (OXZ code registry target), lints, CI
  (3-OS matrix: build + Category A tests; bench job posts numbers; coverage report;
  reference job runs Category B where `structurizr` bin exists), criterion skeleton +
  committed baseline, xtask+oracle v0 (gen from ONE fixture), testdata vendoring script
  + attribution.
- **M1** `ast`+lexer: lossless CST, `expect-test` corpus, preprocess incl. text
  blocks/continuations/BOM (fixtures: text-block, multi-line*, utf8, iso-8859, bom*).
- **M2** `core`: model, tags/ids/canonical names, builder+validation catalog (rule
  fixtures pass), unit suites.
- **M3** `json`: reader leniency matrix + writer with WRITE-CATALOG (P1-D6 first!);
  goldens byte-match on simple fixtures; read-corpus loads; `--keep-unknown`.
- **M4** `dsl` parser+builder: model scope → views/styles/configuration → deployment
  → dynamic/parallel/animation; fixture-by-fixture burn-down; multi-error snapshots;
  implied relationships; archetypes; identifiers modes; portability matrix.
- **M5** directives: `!include` (file/dir), `!const`/`!var`+`${}`, `scope shared`,
  `workspace extends` (file only), `!identifiers`, `!impliedRelationships` on/off,
  finders, `!element(s)`/`!relationship(s)`, `!docs`/`!adrs` statement-form parse +
  OXZ-5003 deferred reject (importers → P4), URL/`!script`/`!plugin`/`!components`
  clean rejects; **M5 complete = full-fixture parse gate (minus deferred docs/ADR
  corpus)**.
- **M6** `fmt` + invariants (incl. proptest), `reconcile`, `validate` polish, CLI
  flags/exit codes, miette rendering, suggestions; cargo-fuzz targets with fixture seed
  corpus running in CI (nightly, time-boxed runs).
- **M7** hardening: perf sanity (parse whole corpus < 100 ms in release — informational),
  docs (`--help`, README quickstart), tag v0.1.0.

## P1-D12 Verification list (empirical, before the code that depends on it)

- V-1 Java array iteration/TreeSet comparators per JSON path → WRITE-CATALOG.
- V-2 per-field omission quirks (e.g. ElementView `z == 0`) → WRITE-CATALOG.
- V-3 actual default of `structurizr.feature.dsl.environment` (env fallback on/off) →
  match it with `--env` default.
- V-4 auto view-key generation exact rule (`Prefix-NNN` numbering incl. collisions)
  from `views-without-keys.dsl`.
- V-5 `!include` of a directory: file set/order/extensions.
- V-6 ~~`!docs`/`!adrs` exclude pattern semantics~~ — **moved to P4-V8** (importers
  deferred 2026-11-14).
- V-7 canonical-name form for custom elements and instances (docs vs code drift).
- V-8 ISO-8859 fixture: Java read encoding (default charset?) → choose identical
  behavior; else explicit error. Also pins newline handling: what CRLF input produces
  in text fields and in the `structurizr.dsl` property bytes.
- V-9 ~~decision-status/date parsing corners per ADR tool format~~ — **moved to P4-V7**
  (importers deferred 2026-11-14).
- V-10 ✅ (2026-11-14, oracle runs, plan/PROGRESS.md) The app layer DOES mutate the
  workspace during `export`: `AbstractCommand.loadWorkspace` runs (a) scope validation
  (hard error), (b) `new DefaultInspector(workspace)` which injects four
  `structurizr.inspection.{error,warning,info,ignore}` count properties into
  `workspace.properties`, and (c) `createDefaultViews()` **iff model non-empty AND no
  views defined** (landscape + context/container per name-sorted system + component per
  container + deployment views). `dsl2json` must mirror (c) (small, specced in
  `ViewSet.createDefaultViews`); (b) is a harness artifact — goldens strip it
  (`xtask::normalize_reference_output`). Parser default workspace name/description are
  the literals `"Name"`/`"Description"` (`WorkspaceParser.java:28`), *not* empty.
- V-11 (M3, before the writer) `properties`-style free-form maps are serialized in
  **Java HashMap iteration order** (hash-bucket order of the string keys, capacity
  power-of-2 ≥ n/0.75, chain = insertion order within a bucket), NOT sorted and NOT
  input order — proven from reference output (PROGRESS.md 2026-11-14). POJO fields are
  declaration-order (happens-alphabetical). Decide: replicate `String.hashCode`+HashMap
  ordering in the writer (deterministic; needs reference insertion order per object —
  WRITE-CATALOG), or canonicalize key order JSON-aware in the parity comparator.
  Empty objects print `{ }`, empty arrays `[ ]`.
- V-12 Date/volatility fields: export stamps NOTHING volatile — `lastModifiedDate`/
  `lastModifiedAgent`/`lastModifiedUser` only round-trip from JSON inputs
  (`lastModifiedUser: ""` omitted as empty; unknown `revision` dropped). Goldens are
  committed raw minus V-10(b) keys.
Each gets a checkbox + evidence link in the PR that implements it; results folded back
into `plan/*.md` if notes were wrong (errata process).

## Risks

- R-1 write-order/omission surprises (mitigation: M3 order — catalog before code;
  goldens catch rest).
- R-2 parser fidelity on weird-but-legal inputs (mitigation: fixture burn-down + fuzz).
- R-3 ADR importer drift — transferred to Phase 4 with the importers (mitigation stays:
  real-world fixture corpora vendored in P4; P4-V7).
- R-4 `${VAR}` pre-pass error-line mapping regressions (mitigation: mapping table +
  snapshots).
- R-5 scope creep — resolved by the 2026-11-14 rebalance: `!docs`/`!adrs` importers
  moved to Phase 4 (statement form still parsed in P1; OXZ-5003 in the interim, D7).
