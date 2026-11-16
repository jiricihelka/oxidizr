# PROGRESS.md — implementation work journal

Living notes for the implementer (survives context loss). Newest entries on top.
Authority order: `DEVIATIONS.md` (contract + Locked) > `build/PHASE_N.md` > this file
(journal/notes only — decisions belong in the phase files).

## Status board

| Phase | Milestone | State |
|---|---|---|
| P1 | M0 scaffold | **done 2026-11-14** |
| P1 | M1 ast+lexer | **done 2026-11-14** |
| P1 | M2 core model + validation | **done 2026-11-14** |
| P1 | M3a write catalog (P1-D6) | **done 2026-11-14** (commit 7848c5e) |
| P1 | M3b json writer + reader | **done 2026-11-14** (commit 0e77dc2) |
| P1 | M4 dsl parser + builder | **done 2026-11-15** (commit 41b70ca) |
| P1 | M5 workspace extends | **done 2026-11-16** (commits 9f174e6, c468854) |
| P1 | M6-a CLI surface | **done 2026-11-16** (log below) |
| P1 | M6-b formatter | **done 2026-11-16** (log below) |
| P1 | M6-c … M7 | M6-c `reconcile` + validate polish next |
| P2 | exporters | pending |
| P3 | viewer+layout | pending |

Commit cadence (user decision revised 2026-11-15, after M4): **commit every green
sub-piece**, wrapped up by a milestone commit (PROGRESS log + status board); keep
`agents/notes/RESUME.md` current at each commit; milestones touching >4 modules are
decomposed in the PHASE doc first. Questions only on plan-contradicting reality
(AGENTS.md working style).

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

- **Repo reorganization (2026-11-15, post-M4)**: all permanent agent files moved under
  `agents/` — `plan/` → `agents/plan/` (git mv, every `plan/` cross-ref in 30+ files
  rewritten), plus a new `agents/notes/` tree of short operational-recall notes
  (env / pitfalls / testing / process, README index per directory) to survive context
  compressions during long milestones. `agents/plan/reports/` (user-dropped CI/OS
  reports) stayed untracked; it was briefly lost when `git mv` removed the emptied
  `plan/` dir and was restored byte-identical from `~/.local/share/Trash` — lesson
  recorded in `agents/notes/pitfalls/git-mv-deletes-untracked.md`. Same day,
  follow-up: AGENTS.md slimmed to a per-task charter; permanent detail moved to
  `agents/plan/COMPAT.md` (contract + sources of truth), `agents/plan/WORKFLOW.md`
  (env, commands, CLI policy, quality/style) and `agents/plan/TESTING.md`
  (categories, goldens, benches); all `AGENTS.md §…` references across the repo
  repointed. Same day: M4 retrospective adopted three process rules — commit every
  green sub-piece (not one commit per milestone), the `agents/notes/RESUME.md`
  checkpoint ritual, and a milestone size gate (>~4 modules ⇒ decompose first).
  No code, JSON-contract, or Locked-list impact.
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
- **Windows CI failures — checkout EOL conversion RESOLVED (2026-11-15)**. Symptom
  (`plan/reports/windows_test_error.txt`, scratch file since deleted): the 6 byte-exact `expect!` CST snapshots in
  `crates/oxidizr-dsl/tests/m1_lossless.rs` failed on Windows with LF-vs-CRLF diffs and
  shifted byte offsets. Root cause is NOT the lexer (lossless corpus tests passed):
  stored blobs are LF, but the Windows checkout had `core.autocrlf=true`, which rewrites
  LF→CRLF at checkout. Fix: root `.gitattributes` — `* text=auto eol=lf` for sources,
  `testdata/** -text` (byte-exact in BOTH directions: fixtures/goldens/assets must never
  be EOL-converted, re-vendoring keeps reference bytes). `.gitattributes` outranks
  `core.autocrlf`, so checkouts are byte-stable on all 3 OSes. Verified by cloning with
  `-c core.autocrlf=true`: fixtures/goldens materialize CR-free, jar byte-identical,
  `m1_lossless` 9/9 green in that clone; `git add --renormalize` was a content no-op.
- **Local Windows leg via Wine (2026-11-15, dev-env decision)**. The dev flake now
  approximates the windows-latest CI leg on Linux: `cargo test --workspace --target
  x86_64-pc-windows-gnu` inside `nix develop` (shell sets the per-target `_RUNNER=wine`
  and a `_RUSTFLAGS=-L native=<libpthread stub>`). Verified: **259/259 green under
  Wine** at P1-M4. Three non-obvious pins, each commented in `flake.nix`:
  (1) `rust-mingw` must ride the rust-overlay `targetExtensions` override — host
  `extensions` rejects it (rustup installs it implicitly with the target);
  (2) nixpkgs' win32-threads mingw omits winpthreads, so std's `-l:libpthread.a` is
  satisfied by symlinking rust-mingw's own stub from `self-contained/` onto the link
  path (a whole-dir `-L` there would shadow the nixpkgs mingw libs);
  (3) the `pkgs.wine64` loader binary is named `wine`, not `wine64`.
  CI stays the authority; Wine does NOT simulate the Windows *checkout* (EOL/autocrlf —
  use the `core.autocrlf=true` clone above), NTFS semantics, or MSVC. macOS remains
  CI-only (no darwin cross from Linux).

## Milestone log

### P1-M6b — `fmt`: canonical formatter on the lossless CST ✅ (2026-11-16)

P1-D7 printer in `oxidizr_dsl::fmt` (NOT the CLI — P1-D1 topology; the CLI
only adds stdin/file plumbing + `--write`/`--check`), formatter never validates.

- Printer walks `cst::build` root children paired 1:1 (cursor) with the
  `preprocess::preprocess` logical lines; 4-space `INDENT`; brace depth from
  token VALUES (a quoted `"{"` opens a context in the reference too — quotes
  are stripped before its scope checks); blank runs compact to 1; whole-line
  comments re-indent, block-comment region interior + text-block raw +
  dropped EOF tail byte-verbatim; conservative unquoting (`unquote_value`:
  terminated, no `"`, no trailing `\`, no whitespace, not `#`/`//`/`/*`-
  starting, not `*/`-ending); LF terminators.
- Line-end guards (→ `agents/notes/pitfalls/fmt-line-end-guards.md`): `"""`-
  ending body → protective space; `\`-ending body → original bytes verbatim
  (unrepresentable — the reference always re-joins); content-final `\r`
  doubled before the LF (`split("\\r?\\n")` is the contract, `parse(File)`
  `readAllLines` is NOT); between-segment-after-terminator whitespace is the
  joiner indent `stripLeading` drops — segment-membership test, not a
  prev-leaf heuristic.
- Gates (`tests/m6_fmt.rs`): corpus idempotence + invariant-1 workspace
  equality (JSON bytes with the base64 `structurizr.dsl` line dropped — it
  legitimately carries the formatted source, P1-D7 invariant 3 — and UUID-
  shaped values redacted, `uuid_v4` is volatile) over ≥90 UTF-8 corpus
  members; proptest idempotence + logical-surface preservation (20 000 cases
  × 400-char DSL-flavoured input green; `.proptest-regressions` seeds kept).
- Two latent M1 bugs fixed on the way (`lexer::lex_comment` mis-tiled a
  comment joined onto an all-whitespace `\` segment; regression test
  `comment_after_all_ws_continuation_segments`).
- CLI `fmt`: default stdout · `--write` in place (only changed files,
  mtime-stable) · `--check` lists unformatted + exit 1 · `--write` `-`
  usage exit 2 · CLI test 15→20 e2e. `oxidizr-ast` dropped from CLI deps
  (formatter consumes the CST inside `oxidizr-dsl`). Refs scratch dir
  `agents/plan/reports/` pinned with a self-ignoring `.gitignore`.

### P1-M6a — CLI surface: dsl2json / json2json / validate ✅ (2026-11-16)

P1-D8 contract implemented end-to-end; `fmt`/`reconcile` keep their plan-ref
bails (M6-b/c).

- `oxidizr-cli`: commands wired to `parse_dsl` + `oxidizr_json` (engine
  `ParseOptions::json_loader` injected with `oxidizr_json::from_slice` —
  P1-D1 topology intact, `workspace extends <*.json>` works through the CLI);
  `-` stdin/stdout; strict UTF-8 (V-8 closed: no `--encoding`); canonicalized
  file path handed to the engine so reference `parse(File)` semantics hold.
- `report.rs`: `Issue` (+boxed `Snippet`) / `Renderer` over miette 7
  `GraphicalReportHandler`; `DslError` → reference exception text re-assembled
  (`<msg> at line N of <file>`) + `source_line` snippet; `JsonError::Parse`
  maps serde line/col to a byte offset; `NO_COLOR`/`--no-color` → ascii theme.
- Exit codes: 0 ok · 1 diagnostics (errors; `--strict` adds warnings) ·
  2 usage (clap) · 3 I/O (`OXZ-5006` incl. non-UTF-8, undetectable format).
- D3 `--keep-unknown`: new `oxidizr_json::roundtrip_keep_unknown` (`keep.rs`)
  overlays the input `Value` onto the writable `Json` tree — unknown,
  legacy (`revision`, `enterprise`, …) and `NON_EMPTY`-omitted members are
  re-appended verbatim after the known keys (sorted); arrays pair by
  `id`/`key`/`selector` identity, then compatibility (the writer re-sorts).
  `printer::Json::Raw` carries non-`i64`/null literals byte-verbatim. Idempotent;
  no-extras input reproduces the parity bytes exactly.
- New codes registered (`docs/diagnostics.md`): `OXZ-3001` validation,
  `OXZ-4001/4002` JSON read/write, `OXZ-5006` CLI I/O/policy.
- Gates: `tests/cli.rs` 15 e2e tests — golden byte-exact stdout, JSON-base
  loader injection, keep-unknown against read-corpus legacy keys, exit codes
  0/1/2/3, miette text checks (`OXZ-…`, reference text, snippet line); quartet
  green; `oracle verify` 4/4 (printer/writer untouched in parity output).
- Test-writing pitfalls worth remembering (→ `agents/notes/testing/
  cli-e2e-pitfalls.md`): `wait_with_output` needs explicit `Stdio::piped()`;
  `structurizr.dsl.identifier` is a fresh random UUID per parse (normalize it
  in stdin↔file comparisons); `lastModified*` are KNOWN round-tripping fields
  (`revision` is the legacy one); relative `workspace extends`/`!include`
  must not appear in stdin-vs-file fixtures (no file context — reference-
  faithful skip).

### P1-M5 — `workspace extends` (file + JSON bases) ✅ (2026-11-16)

Reality note first (PHASE_1 §P1-D11): everything else in M5's original scope
had already landed with M4, so M5 = `workspace extends` only; decomposed
M5-a/b/c.

- **M5-a** `core`: `WorkspaceBuilder::adopt` — replaces the workspace under
  construction (reference `WorkspaceParser.parse` REPLACES the object) and
  replays every element+relationship id through `IdGenerator::found`
  (`Model.add` seeds the counter on every add, `Model.java:274/286`);
  `Properties::remove` (Java `HashMap.remove` semantics — slot order is
  part of the write contract, V-11) + `remove_property`. Commit 9f174e6.
- **M5-b** `dsl` engine: `Parser::extend_from` per `WorkspaceParser.java`
  @ad34036 — URL sources → OXZ-5001 with the reference feature texts;
  non-file parse skips silently (`context.getFile() == null` quirk);
  `… could not be found` / `… should be a single file` guards; `.json`
  bases via injected `ParseOptions::json_loader` hook (topology kept: no
  `json` dep in `dsl`; CLI wires it, tests use it as dev-dep) with
  `registerIdentifiers` (elements AND relationships carrying
  `structurizr.dsl.identifier`); DSL bases via a child `Parser` whose
  register merges into ours (`DslParserContext.copyFrom` — verbatim-merge
  rationale in `idents.rs`), `configureFrom` (identifier scope, archetypes,
  constants/variables), then `adopt`; `clearDsl` + `dsl_portable = false`
  (extending file keeps no source — `DslTests` `getDsl == ""`);
  `extending_workspace = !model.isEmpty()` (people+systems+deploymentNodes)
  drives get-or-add-by-name reuse in the four element parsers
  (`isExtendingWorkspace` branch; the registration collision guards
  inherited from M4 reproduce the reference throw-for-throw).
- **M5-c** gates: OXZ-5004 → tombstone (`docs/diagnostics.md`, constant
  removed); 11 `EXTENDS_DEFERRED` burn-in rows retired — 9 file fixtures
  parse CLEAN, the 2 URL fixtures re-registered under D5 (OXZ-5001,
  `Extends via HTTPS are not permitted`); `m5_extends.rs` 7 Category A
  tests (layering, hierarchical `configureFrom` witness, JSON-base
  identifiers, 3-level chain, no-retention + id continuation, loader-absent
  guard, file guards, JSON-base element reuse); golden `extend/4.dsl`
  byte-exact (`m5_goldens.rs`).
- `xtask oracle`: normalization drops the empty `properties` shell left by
  inspection-property deletion; `find_fixture` resolves JSON fixtures
  (`oracle verify` worked on `m3b-defaults` for the first time since M3 —
  all 4 goldens verify green); `agents/notes/testing/oracle-app-harness.md`
  records why viewless fixtures cannot be goldens (V-10 `createDefaultViews`).

Quartet green (`cargo xtask ci`); `oracle verify` 4/4; nothing pushed.

### P1-M4 — `oxidizr-dsl` engine: model + views + styles + deployment + dynamic ✅ (2026-11-15)

- The engine (`crates/oxidizr-dsl/src/engine/`) is a faithful port of the
  reference single-pass parser: per logical line → tokenize → `${}`-substitute
  → `id =` strip → the ordered `else if` chain over the context stack. The arm
  ORDER is the compatibility contract — every arm carries its
  `StructurizrDslParser.java` line number (@ `ad34036`); D1 per-line error
  recovery truncates the failing line's token tail and keeps parsing.
- Lint policy for the port: mechanical lints get real fixes (`strip_prefix`,
  let-else, `clamp`, merged arms, …); faithful-port shapes get justified
  `#[allow]`s (`too_many_lines` on the chain sections and the per-context
  permitted-token tables, `match_same_arms` where distinct reference classes
  collapse, the `tok.rs` keyword table kept whole for M5+). Workspace-wide
  `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- **Burn-in harness**: `crates/oxidizr-dsl/tests/m4_fixtures.rs` walks every
  `.dsl` under `testdata/dsl` (107 files); each must parse CLEAN or match the
  `KNOWN_ERRORS` register (first-error OXZ code + message prefix), checked in
  BOTH directions so retired deferrals must be pruned here. 58 clean +
  48 registered: 11×OXZ-5004 `workspace extends` (retires at M5), 4×OXZ-5003
  D7 docs/ADRs (P4), 3×OXZ-5005 D11 plantuml imports (P4), 2×OXZ-5001 D5 URLs,
  8×OXZ-5002 D2/D10 JVM rejections, 12 reference-asserted failures with pinned
  messages (`DslTests.java`), 7 include/extends component files, and
  `image-view.dsl` (corpus vendors no `image.png`). `iso-8859.dsl` has its own
  Latin-1 test mirroring `test_ISO8859Encoding` (`Namé` loads).
- Bugs the burn-in caught (fixed + unit-pinned):
  1. `WorkspaceBuilder::dsl_uses` rejected ANY second relationship between the
     same pair. Reference: `uses()` returns null (→ the "already exists"
     throw) only on same destination AND same description — different
     descriptions add fine (`filteredviews`, `bulk-operations`,
     `dynamic-view-with-explicit-relationships`).
  2. `!element <deploymentEnvironment> {` opened no context. The DSL-package
     `DeploymentEnvironment extends Element` shim registers under the `de =`
     identifier and the finder ladder pushes a fresh
     `DeploymentEnvironmentDslContext(name)` (`deployment-environment-empty`).
- Reference-quirk errata, each verified against Java @ `ad34036` and pinned by
  engine tests (all byte-compatible with the reference; NOT deviations):
  - view-definition / `animation` / `terminology` arms push their context
    UNCONDITIONALLY (a no-`{` definition still swallows following lines).
  - `endParallelSequence(!relationships)` (`DynamicViewParallelSequenceDslContext
    .end()`): a NON-empty parallel block DISCARDS the child counter — the next
    top-level step reuses the number (orders `["1","2","2"]`); an empty block
    burns one number instead.
  - a `dynamic <system>` view can NOT add the scope system itself
    ("`X is already the scope of this view and cannot be added to it.`");
    unscoped `dynamic *` steps accept only Person/SoftwareSystem; pair steps
    require an EXISTING model relationship.
  - explicit `N:` step orders burn one sequence number (the next implicit step
    continues from the counter, not from `N`).
  - `a -> b {` inside a dynamic view consumes `{` as the description
    (JSON: `"description": "{"`).
  - svg content inside `light { }`/`dark { }` blocks stores PLAIN `content`
    (the reference never fills `contentLight`/`contentDark` from the DSL).
  - `theme <file>` and `image <file>` file branches need a file-based parse
    (`ParseOptions::file*`); against a fragment the reference NPEs and we emit
    the same failure path.
  - `terminology relationship Connects to` stores `tokens.get(1)` verbatim —
    the unquoted second word is dropped (`Connects to` → `Connects`).
  - `InstalledThemes` is app-layer: `theme <name>` stores the name verbatim;
    the file-branch existence check is reference-exact (`Theme X does not
    exist at <dir>/X`).

### P1-M3b — `oxidizr-json`: byte-parity writer + lenient reader ✅ (2026-11-14)

`oxidizr-json` per the amended P1-D5: hand-rolled writer (`Json` tree → Jackson-shaped
printer + `javahash` `HashMap` emulator) and a `serde_json::Value`-walk reader (with
`preserve_order` — document-order property maps are the precondition for byte-stable
round-trips). Every WRITE-CATALOG §1–§5 rule is explicit code (sort functions, inclusion
per class, computed view names, instance name suppression); `Workspace → bytes` and
`bytes → Workspace` now bracket the compatibility contract:

- **Writer goldens byte-identical**: `m3-order.json` + `workspace-properties.json`
  re-written from the public core API (key tables, all §5 sorts, HashMap property order,
  `{ }`/`[ ]`, no trailing newline). `golden_m3.rs`.
- **Reader**: §6 leniency matrix (unknown-ignore, coercions, aliases incl. the
  document-order `enterpriseContextViews` replacement, `theme`→`themes` + `addTheme`
  dedupe), hydrate-equivalent validation (`JsonError::Validation` carrying the FIRST
  reference message), instance `name`/`description` derived after the link checks.
  `read-corpus/` (5) loads; `reader_m3.rs` + 15 tests.
- **Reader probes vs the oracle (2026-11-14)** — committed as
  `testdata/probes/m3b-defaults.json` + its golden (read→write byte-identical witness;
  probe inputs must stay re-readable — a healthCheck with null `name`/`url` NPEs the
  reference READ via `HttpHealthCheck.hashCode()` on Jackson's first HashSet insert).
  Settled: healthCheck `headers` write in **HashMap bucket order** replayed from
  TreeMap-sorted insertion (`getHeaders()` copies — writer uses `javahash::entry_order`;
  `{"X-A","X-B"}` writes `X-B` first); `name`+`url` required on read; `interval`/
  `timeout` read defaults 0/0 (60 is DSL-only); `automaticLayout` read defaults
  0/0/0/false + NULL `rankDirection`, and a PRESENT-but-unparsable `rankDirection`
  throws `A rank direction must be specified.` (the only guarded enum setter);
  style colors normalized by the `Color` setters — vendored 148-name table
  (`colors.rs`), `#rrggbb` lowercased, else `<x> is not a valid hex colour code or
  HTML colour name.` (M4/M5 DSL side must apply the same normalization).
- **Core faithfulness**: `AutomaticLayout.rank_direction`, `FilteredView.mode`,
  `Section.format`, `Decision.format`, `User.role` became `Option` (null bean fields —
  unknown/absent JSON enums bind null, the writer omits the key; builder wraps DSL
  values in `Some`). `users` DO round-trip (old catalog claim corrected in §6 errata).
- **Validation-corpus erratum**: per the reference `WorkspaceRulesValidationTests`,
  15/16 fixtures throw and `TopLevelDeploymentNodeNamesAreNotUniqueButTheyExistIn…`
  must LOAD (uniqueness is per name+environment). The harness now pins every first
  message against the Java test strings verbatim.
- **proptest**: `to_vec → from_slice → to_vec` byte-stable for random property maps
  (the `H(H(I)) == H(I)` idempotence behind JSON→JSON stability).
- Deferred to M6 (CLI milestone): the `--keep-unknown` capture and the `json2json`
  command wiring (D3 opt-in, no default-parity effect) — noted on the PHASE_1 M3 line.
- Gates: 113 tests green, `cargo fmt --check`, `cargo clippy --workspace --all-targets
  -- -D warnings` clean.

### P1-M3a — write catalog (P1-D6) + core model alignment ✅ (2026-11-14)

`agents/plan/build/WRITE-CATALOG.md` is the authoritative JSON write contract (resolves V-1,
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

Core-model facts verified before coding (`agents/plan/FEATURES.md` corrections inline
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
earlier notes in this file were wrong on all four — `agents/plan/DSL.md` must be
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
   scripts — `agents/plan/DSL.md` "no `#` comments" is WRONG; recorded below).

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
`agents/plan/DSL.md` §1 "M1-verified corrections".

Kept (unverified, M4 must confirm against source before relying on them): the
`${VAR}` pre-pass is a per-token, forward single pass over constants + variables +
text blocks with regex `\$\{[a-zA-Z0-9-_.]+?}` (defs must precede uses; unknown
names stay literal unless the env feature is on — V-3 `--env`); text-block values
substitute recursively, constants/variables do not.
