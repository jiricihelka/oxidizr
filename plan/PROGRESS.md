# PROGRESS.md — implementation work journal

Living notes for the implementer (survives context loss). Newest entries on top.
Authority order: `DEVIATIONS.md` (contract + Locked) > `build/PHASE_N.md` > this file
(journal/notes only — decisions belong in the phase files).

## Status board

| Phase | Milestone | State |
|---|---|---|
| P1 | M0 scaffold | **done 2026-11-14** (this commit) |
| P1 | M1 ast+lexer | **done 2026-11-14** (next commit) |
| P1 | M2 core model + validation | **done 2026-11-14** (this commit) |
| P1 | M3a write catalog (P1-D6) | **done 2026-11-14** (this commit) |
| P1 | M3b … M7 | M3b (`oxidizr-json` crate) next, rest pending |
| P2 | exporters | pending |
| P3 | viewer+layout | pending |

Commit cadence (user decision 2026-11-14): **one commit per green milestone**, nothing
in between. Questions only on plan-contradicting reality (AGENTS.md working style).

## Environment facts (verified 2026-11-14)

- `cargo`/`rustc` on PATH = flake's pinned nightly: `cargo 1.102.0-nightly (ede6cefea
  2026-11-11)`, `rustc 1.102.0-nightly (8eb778d04 2026-11-12)`. The session PATH already
  carries the dev shell (rustc, cargo, structurizr, java-less oracle wrapper all work).
- Oracle on PATH: `structurizr` → nix-wrapped war 2026.10.26 (library 6.2.4).
  Invocation contract (verified): `structurizr export --workspace F --format json
  --output DIR` (GNU options only; `-o` is a directory; output file = input basename +
  `.json` inside it). Version line: `structurizr: 2026.10.26 / structurizr-*: 6.2.4`.
- Reference clone (Apache-2.0, pinned `ad34036`): `/tmp/nix-shell.uni100/opencode/structurizr`.
  DSL fixtures live in `structurizr-dsl/src/test/resources/dsl/` (84 top-level `.dsl` +
  subdirs `extend/`, `image-views/`, `decisions/`, `docs/`, `big-bank-plc/`,
  `spring-petclinic/`, `include/`… 113 `.dsl` in repo total — vendor only the dsl dir +
  client read-corpus + validation, see P1-D10).
  Read-corpus: `structurizr-client/src/test/resources/` (`backwardsCompatibility/`,
  `structurizr-36141-workspace.json`…). Validation fixtures: same resources dir
  `workspaceValidation/` (16 JSON).
- V-10 fixture (no views): `workspace-properties.dsl`.
- Reference download host works: `download.structurizr.com` (static.structurizr.com does
  NOT resolve — never use in docs).
- git remote: `git@github.com:jiricihelka/oxidizr.git` (nothing pushed yet as of M0).
- python3 is NOT on PATH; use
  `/nix/store/9zclbh8196695bbjs106wh8c4957n002-python3-3.13.12-env/bin/python3`.

## Working notes / errata found while implementing

- (M0) Fixture path correction: `…/test/resources/dsl/…` (notes-*.md listed bare
  filenames) — vendor script must use the `dsl/` root.
- **V-10 RESOLVED (2026-11-14)** — reference app pipeline around every `export`:
  `AbstractCommand.loadWorkspace` = load → `WorkspaceScopeValidator.validate()` (hard
  error; fixtures failing validation can never have goldens) →
  `new DefaultInspector(workspace)` → `createDefaultViews()` if `!model.isEmpty() &&
  views.isEmpty()`.
  - Default views (V-10 proper): generated for **model non-empty + zero views**
    (`workspace-properties.dsl` has an empty model → none, which is why the first probe
    showed nothing). Shape per `structurizr-core/…/view/ViewSet.createDefaultViews()`
    (class renamed from `Views` in 6.2): landscape `addDefaultElements` + per system
    (sorted by name) context & container views + component view per container with
    components + deployment views per environment×system; empty key ⇒ auto-keyed
    (V-4 numbering). **`dsl2json` must mirror this** (app-parity is the oracle contract);
    keep the *library* generation-free (Locked list).
  - Inspection counters: `DefaultInspector` writes `structurizr.inspection.error/.
    warning/.info/.ignore` (violation counts) into workspace `properties` on EVERY
    load/export. Harness artifact, not file content; whole `structurizr-inspection`
    crate is out of scope to reimplement. Goldens strip these four lines
    (`cargo xtask` `normalize_reference_output`, incl. trailing-comma fixup when they
    sort last); json2json round-trips them as ordinary properties. DEVIATIONS-worthy —
    register there if user-visible behavior solidifies in M3/M4.
  - DSL parser defaults: workspace name/description default to the literal strings
    `"Name"` / `"Description"` (`WorkspaceParser.java:28`), overwritten by the
    `workspace` statement. Builder (M4) must start from these.
- **Properties map serialization = Java HashMap order (V-11, M3 blocker)**. Proof:
  workspace-properties export ⇒ `error, dsl, info, dslEditor, warning`; 36141 JSON→JSON
  ⇒ `info, ignore, error, warning, dsl`. Neither sorted nor insertion-ordered; order is a
  function of key hashes (+bucket chains). Plan correction: P1-D5's "BTreeMap sorts for
  free" is WRONG for byte parity of `properties` (any `properties` object: workspace,
  elements, relationships, views, …). POJO fields remain declaration-ordered (the
  golden's alphabetical top level is POJO field order, that part is fine). Options for
  M3 (decide there, recorded in WRITE-CATALOG): (a) replicate Java `String.hashCode` +
  HashMap bucket walk in a custom serializer, or (b) parity comparator canonicalizes
  key order of `properties` objects JSON-aware. (a) is compatible with byte-goldens;
  (b) weakens the gate. Empty object renders `{ }`, empty array `[ ]`, `"key" : value`.
- **Volatility RESOLVED — export is deterministic**: no timestamp is stamped by the app
  (earlier note about `lastModified` was wrong). `lastModifiedDate/. lastModifiedAgent`
  round-trip from JSON inputs verbatim; `lastModifiedUser: ""` omitted (empty-string
  omission); unknown keys (`revision` in cloud exports) dropped; compact input JSON is
  rewritten in Jackson pretty layout; numeric `workspace.id` preserved (36141 test).
  So read-corpus goldens need no normalization.
- `loadWorkspace` also `installThemes`(local `themes/` dir or `$STRUCTURIZR_THEMES`) —
  theme URLs are fetched during PNG/SVG export only, not `--format json`.

## Milestone log

### P1-M3a — write catalog (P1-D6) + core model alignment ✅ (2026-11-14)

`plan/build/WRITE-CATALOG.md` is the authoritative JSON write contract (resolves V-1,
V-2, V-11). Built by (a) reading `structurizr-client` `io/json/JsonWriter`/`AbstractJson*`
and every `structurizr-core` POJO getter (Jackson `SORT_PROPERTIES_ALPHABETICALLY` +
`NON_EMPTY`), and (b) a single oracle probe `testdata/probes/m3-order.dsl` exported
twice (`cmp` byte-identical → deterministic) whose raw output is the byte witness
`testdata/golden/m3-order.json`.

Headline findings (all verified, not recalled):

- **V-11 resolved: replicate Java `HashMap` bucket order exactly** — free-form
  `properties` are NOT sorted. Core `properties` moved from `BTreeMap` to a new
  insertion-ordered `oxidizr_core::properties::Properties` (the writer replays insertion
  order through a `HashMap` emulator). Two pathological edge cases accepted (tree bins,
  inspection-key capacity) → `DEVIATIONS.md` D8.
- **V-2 resolved: primitive ints/bools are NEVER "empty"** — `x`/`y` (`0`),
  `instanceId`, `enterpriseBoundaryVisible` (`true` default), `*BoundariesVisible`
  (`false`) are ALWAYS written. `generatedKey` only when `true` (`NON_DEFAULT`);
  RelationshipView `response` (`NON_NULL`) writes `false`.
- Computed view `name` is always written (exact strings per kind, §3.1); instances
  serialize with NO name/description; DynamicView `deploymentEnvironment` is invisible
  both directions (private, no Jackson accessors); `filtered` legacy `filter` key is
  read-IGNORED (no setter).
- Array order (V-1): element arrays numeric-id ascending; view lists `(order, key-ci)`;
  relationship views `id/order`; filtered tags & style sets UTF-16 `String.compareTo`.
- Core model aligned to the catalog: `Properties` everywhere; `metadata` on
  CustomElement; static-view `animations`; landscape/context `enterpriseBoundaryVisible`
  default **true**; dropped `RelationshipView.technology`, `FilteredView.legacy_filter`;
  `@JsonIgnore` on ANY accessor suppresses the whole Jackson property (the mystery of
  the invisible `name`/`mergeFromRemote` getters, resolved).
- DSL grammar discoveries logged in WRITE-CATALOG §8 (filtered/deployment view
  signatures, `deploymentGroup` declaration vs reference semantics, no async arrow,
  Landscape scope rejects containers, parent↔child relationships throw at parse).

`xtask oracle` fixes surfaced by the new golden: `normalize_reference_output` preserves a
missing trailing newline (M0 goldens had an artifact `\n` — the reference emits none), and
`find_fixture` now also scans `testdata/probes`. Both goldens regenerated EOF-correct.

### P1-M2 design notes (2026-11-14, from reference source `ad34036`)

Core-model facts verified before coding (`plan/FEATURES.md` corrections inline
in its tables where noted):

- **Ids are strings in the model**, not ints: `ModelItem implements Comparable`
  with `Integer.parseInt` both → numeric compare, else string compare;
  `Model.getElement(id)` is a Java `HashMap<String,…>` (TEXT equality lookup);
  legacy workspaces may carry non-numeric ids and
  `SequentialIntegerIdGeneratorStrategy.found()` silently ignores those.
  → `core::ids::Id { text: Arc<str>, num: Option<i32> }`; Eq/Hash/Ord = text;
  numeric view cached for the counter bump and the M3 TreeSet-order comparator.
- **Tags**: `getTags() = LinkedHashSet(getDefaultTags() ++ stored)`; the stored
  field keeps only custom tags in memory but JSON loads re-store the defaults
  (harmless dedup on write). `setTags(csv)` = `split(",")` NO trim (bug);
  `addTags` trims. Relationship defaults are STATEFUL: linked rel → NO defaults;
  else `Relationship` (+`Synchronous`/`Asynchronous` if interactionStyle set).
  Instances (`SoftwareSystemInstance`/`ContainerInstance`): NO inherited-parent
  tags in v6 — FEATURES §3 "parent system's tags + …" is OUTDATED; the ctor
  stores just the instance tag, and JSON load keeps exactly the tags string.
- **Canonical names are URI-style** (FEATURES §2 `Person[X]` table is OUTDATED):
  `Person://X`, `SoftwareSystem://S`, `Container://S.C`, `Component://S.C.K`,
  `Custom://X`, `DeploymentNode://ENV/A/B`, `InfrastructureNode://ENV/A/B/N`,
  `SoftwareSystemInstance://ENV/A/B/System[instanceId]`,
  `ContainerInstance://ENV/A/B/Sys/Container[instanceId]` (note: the node-chain
  drops the `DeploymentNode://` prefix; instanceId is the instance's OWN int,
  the static link is `softwareSystemId`/`containerId` — FEATURES §2 "instances
  link via instanceId" is OUTDATED). Every name part has `.` and `/` REMOVED
  (`formatName`) before joining; static separator `.`, deployment separator `/`.
- **v6.2.4 field truth** (JSON-FORMAT "verify" items): no `background` on View;
  ElementView = `{id,x,y}` only (+nothing — no width/height/z/orientation);
  RelationshipView = `{id, description, url, order, response, vertices[],
  routing, jump, position, properties}`; `numberOfInstances`/`dependencies`
  DO NOT EXIST; health checks live on INSTANCES only (not nodes/infra nodes);
  DeploymentNode has `instances` (int in JSON), `technology`, `deploymentGroups`,
  `platform`/`usage` DO NOT EXIST; animations only on DeploymentView+CustomView;
  FilteredView `tags` is a TreeSet (serialized ALPHABETICAL); workspace-level
  `configuration` = `{scope?, visibility?, users?}` (v6 `WorkspaceConfiguration`,
  legacy `branding`/`styles` inside top-level `configuration` in old goldens are
  IGNORED by the reader — `views.configuration` is the real styles home);
  `ImageView` extends View (NOT ModelView — no dimensions/elementViews);
  DynamicView has `elementId` + `deploymentEnvironment` + `externalBoundariesVisible`.
- **Validation catalog** = `Model.hydrate()` then `ViewSet.hydrate(model)`
  (messages + order transcribed in `core::validate`, fixtures
  `testdata/validation/*.json` + expected messages from
  `structurizr-client` `WorkspaceRulesValidationTests`). Model checks run in
  hydrate order: unique element ids → unique relationship ids → instance static
  links → person/system name uniqueness → containers → components → top-level
  nodes (per environment) → recursive children → rel description uniqueness
  (per source, by (destination, description)) → linked-rel ids exist. ViewSet
  checks: view-key uniqueness (walks a TreeSet ordered by (order, key-ci), the
  `ViewKeysAreNotUnique` fixture relies on distinct `order` to survive the set) →
  per-view-type element links → element/relationship view entries exist →
  filtered base view exists and is static/deployment → image view element.
  First violation aborts (reference throws); we collect ALL (D1) but keep rule
  ORDER stable.

### P1-M2 — core domain model + validation catalog ✅ (2026-11-14)

Landed (`cargo test -p oxidizr-core` green on the first run — the design notes
above held, no errata):

- `crates/oxidizr-core`: ten modules (`ids`, `tags`, `canonical`, `model`,
  `views`, `docs`, `configuration`, `error`, `validate`, `builder`) over a
  dependency-free core (P1-D1; `serde_json` is a **dev**-dependency for the
  fixture test only). `lib.rs` hosts the workspace default name/description
  consts plus `DEFAULT_ENVIRONMENT`/`DEFAULT_INSTANCES`.
- 20 unit tests + `tests/validation_catalog.rs`: a deliberate mini-reader over
  `serde_json::Value` (flattens nested `relationships` arrays like
  `JsonReader` + `Model.hydrate` do — M3 replaces it) loads all 16 vendored
  `testdata/validation/*.json` fixtures; each asserts the EXACT
  `WorkspaceValidationException` message `WorkspaceRulesValidationTests`
  expects, the "different environments" fixture asserts NONE, and
  `every_fixture_is_covered` fails if a fixture appears without an expectation.
- Full gates: 78 workspace tests, `cargo fmt --check`, clippy `all`+`pedantic`
  under `-D warnings` all clean.

Facts verified against the reference **while coding** (all now encoded in
`validate.rs`/`builder.rs` with source references):

- Validation catalog = `Model.hydrate()` then `ViewSet.hydrate(model)`;
  per-view check order custom → landscape → context → container → component →
  dynamic → deployment → filtered → image; filtered views get NO
  element/relationship entry checks; every violation is collected (D1) and the
  FIRST message is what the reference throws.
- Exact strings verified (fixtures assert them): `A view with the key key
  already exists.` (key unquoted), the filtered-view base-view message
  (`(key=SystemContext)` unquoted), the instance-link messages
  (`(id=1) … that does not exist in the model`).
- Multiple violations: the reference's exact winner depends on Java `TreeSet`
  iteration order — with one violation per fixture our first message is
  byte-identical; noted in `validate.rs` header for multi-violation work.
- Builder API names follow the reference (`add_person`, `add_relationship`, …);
  `IdGenerator::generate` (not `next`) to avoid `should_implement_trait`;
  the interim view-order rule is `views.size()+1` (V-4 revisits it in M4).

### P1-M1 — DSL lexer + lossless CST ✅ (2026-11-14)

`oxidizr-diag` (Span/FileId/Code/Collector), `oxidizr-ast` (lossless
`Cst`/`Node`/`NodeKind` + byte-coverage `verify_lossless`), and the
`oxidizr-dsl` front stages: `preprocess` → `lexer` → `cst` (M4 adds the
statement parser on top).

Pipeline shape (P1-D3 realized):
- `preprocess::preprocess(source) -> Vec<Logical>` — the reference's
  `preProcessLines`: physical lines are byte-spanned (`\r?\n`, EOF),
  `\\`-continuations join into segment lists, text blocks fold into a
  `TextBlock` raw region, EOF tails of unfinished groups are marked `dropped`
  (the reference discards them).
- `lexer::lex_line/comment/blank` — bug-compatible `Tokenizer.java` over the
  UNtrimmed bytes: token VALUES equal the Java tokenizer's, leaves tile every
  physical byte (whitespace/newline/continuation/BOM trivia; token hull leaves
  include quote delimiters, multi-segment tokens produce multi-leaves with
  exact pieces).
- `cst::build` — classification + block-comment state machine (see checks
  below) → flat lossless tree `Root > line nodes > leaves`; `cst::LineIndex`
  gives (line, char-col) for diagnostics (D1).

**Reference-semantics corrections found during M1** (verified against
`ad34036` `StructurizrDslParser.preProcessLines` + `Tokenizer.java`; the
earlier notes in this file were wrong on all four — `plan/DSL.md` must be
re-checked against these when M4 documents statements):
1. **Continuation joins keep indentation**: only the FINAL completing line is
   `stripLeading`ed; middle lines append verbatim (join of
   `a \` / `  b \` / `  c` is `a   b   c`).
2. **Logical lines are emitted under the LAST physical line's number** — the
   parse loop flushes with its already-incremented `lineNumber` (multi-line
   statement errors point at the closing line).
3. **Text-block values include the opener line's post-`"""` text**: the
   reference buffers the WHOLE opener line, so `split(TEXT_BLOCK_MARKER)`
   leaves `parts[1]` = tail-after-first-`"""` + dedented middle lines each +
   `\n`; then `substring(0, len-1)` drops the last char. Value = that.
   Throwing inputs (empty value, middle line shallower than first-line indent)
   are modeled as `text_block_value() -> Option<String>` returning `None`
   (M4 will report them as diagnostics instead of Java's
   StringIndexOutOfBoundsException — DEVIATIONS candidate).
4. **Text-block checks precede blank/comment checks** in the reference loop,
   and the whole-line-comment pattern is `^\s*(//|#).*$` (`#` included, like
   scripts — `plan/DSL.md` "no `#` comments" is WRONG; recorded below).

Other verified facts encoded in the code:
- Block-comment machine check ORDER is load-bearing: whole-line `/* … */`
  neither opens nor closes a region (so it does NOT close an open one); a
  line whose FIRST token (after `x =`-assignment strip, reference L431)
  starts `/*` opens; the next line whose TRIMMED text ends `*/` closes; a
  `//`-comment line's `*/` never closes; unclosed `/*` swallows to EOF.
- Tokenizer: quote chars stripped; `\"` = prev-char-backslash lookbehind with
  the backslash KEPT in the value; `"` mid-bare-token is ordinary; EOL pushes
  the open token silently (unterminated `"` loses its opener, no error);
  `#`/`//` are ordinary tokens mid-line; `String.trim` (≤ U+0020) vs
  `Character.isWhitespace` sets both ASCII here (compat.rs notes the Unicode
  edge).
- BOM (U+FEFF) is stripped per logical line (reference) — covered as a `Bom`
  trivia leaf; `iso-8859.dsl` is the one non-UTF-8 fixture: M1 lexes `&str`,
  byte-level decoding is the M5 loader (V-8), pinned by a test.

D1 deviation (already locked): parser errors will carry LINE + COLUMN (the
reference emits no column); M4 wires `Collector` into the statement parser.

**Gates**: `cargo fmt --check` · `clippy -D warnings` clean · `cargo test
--workspace` = 54 tests green (dsl 34 unit + 9 integration incl. six
expect-test snapshots over multi-line/text-block/utf8/workspace-with-bom*/
multi-line-with-error + two whole-corpus properties: every UTF-8 fixture's CST
is byte-lossless, tokens deterministic & char-aligned).
xtask/clippy note: workspace lints catch `push_str(&format!…)`
(clippy::use_format) and `unused dependency` manifest warnings are NON-BLOCKING
(they stay until M2–M4 use the deps; `-D warnings` does not fail on manifest
warnings — verified).


### P1-M0 (2026-11-14, in progress)

Scope (P1-D11 M0): workspace + 7 crates, `rust-toolchain.toml`+`NIGHTLY.md`, LICENSE
files + metadata, `docs/diagnostics.md` stub, lints, CI (3-OS + bench + coverage +
reference jobs), criterion skeleton + committed baseline, xtask oracle v0 (gen ONE
fixture) + `vendor`, testdata vendored with attribution. flake.nix: DONE (9d0498e).

- Pin: rust-toolchain.toml channel `nightly-2026-11-13` == the flake-provided toolchain
  (rustc version string lags the channel date by 1 day: `…(8eb778d04 2026-11-12)`).
  Both bumped together at milestones, register in NIGHTLY.md.
- rust-version = 1.85 (edition-2024 floor; no nightly features used in P1 code so far —
  see NIGHTLY.md).
- `.cargo/config.toml`: `[alias] xtask = "run --package xtask --"` (matklad pattern)
  makes the mandated `cargo xtask …` spelling work; committed (dev tooling, not product
  config).
- Toolchain quirk (nightly-2026-11-13): cargo now emits manifest-level `unused
  dependency` WARNINGS for declared-but-unused deps (skeleton crates; P1-D9 set declared
  up front). Not errors — `-D warnings` unaffected; each retires when its milestone
  lands the code. Skeleton libs carry `#![allow(unused_crate_dependencies)]` with
  remove-when notes for the rustc lint side.
- xtask fixes made while running it for real: `Path::extension()` has no dot (walk_files
  `.dsl` comparison — counted 0 fixtures); `scan_deferred` must be lossy (iso-8859.dsl
  is not valid UTF-8, V-8).
- Clippy surfaces that needed the honest fix: clap default rename would be
  `dsl2-json` → `#[command(rename_all = "lowercase")]` on the `Command` enum (surface
  names are Locked); `doc_markdown` wants backticks around `PHASE_1`, `NO_COLOR` etc.
- Vendored (107 `.dsl` + 21 JSON): deferred-for-D7 list is exactly
  `dsl/big-bank-plc/model/internet-banking-system/details.dsl`, `dsl/decisions/workspace.dsl`,
  `dsl/docs/workspace.dsl`, `dsl/include/model/software-system/model.dsl` (cross-checked
  vs raw grep). Validation corpus = 16 JSON. read-corpus workspaces carry round-tripping
  `lastModifiedDate*` (deterministic — no golden normalization needed).
- V-10 golden: `testdata/golden/workspace-properties.json` generated through the oracle
  (normalized: 4 `structurizr.inspection.*` lines removed, last-entry comma fixed —
  exercised for real: they DO sort last here) + `cargo xtask oracle verify` → ok.
- M0 gates: fmt / clippy(-D warnings) / test all green on the pinned toolchain; bench
  baseline committed under `bench-baseline/M0/`.

### P1-M1 design notes — SUPERSEDED

The pre-implementation notes written from `ad34036` before coding contained four
errors (indentation-stripping, first-line numbering, text-block value bounds, and
`#`-comment scope), all corrected in the **P1-M1 milestone log above**, which is
authoritative for the implemented semantics. The verified facts are encoded and
tested in `crates/oxidizr-dsl/src/{preprocess,lexer,cst}.rs`; see also
`plan/DSL.md` §1 "M1-verified corrections".

Kept (unverified, M4 must confirm against source before relying on them): the
`${VAR}` pre-pass is a per-token, forward single pass over constants + variables +
text blocks with regex `\$\{[a-zA-Z0-9-_.]+?}` (defs must precede uses; unknown
names stay literal unless the env feature is on — V-3 `--env`); text-block values
substitute recursively, constants/variables do not.
