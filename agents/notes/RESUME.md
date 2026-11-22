# RESUME — session checkpoint (updated 2026-11-22, **P4 §P4-M0…M3 CLOSED; §P4-M5 E-1…E-3 DONE (M5a/b/c); P4T **CLOSED** 2026-11-22 (matrix frozen, no tag); §P4R **R0+R1 DONE** (R2 next → R3 `Request` rework → R4 → R5) → P4D → release tail M5d-e (cargo-dist + `v0.4.0` tag + changelog)**)

## Active now (2026-11-22)

- **§P4-M5 E-1…E-3 DONE**: M5a `oxidizr oracle` user command; M5b
  plantuml/mermaid semantic comparators (Category B 31/31 + 31/31);
  M5c `Layered` channel-lane pass `spread_channels` (+ rank-tightening
  proven already-implemented; boundary refinement DEFERRED as an
  additional-feature candidate — all three verdicts in the PHASE_4 §P4-M5
  item stamps, incl. the bench A/B evidence: lane pass ≈ +8% only on the
  1k-node `mixed` shape, chain/star neutral; suite-wide drift vs the
  `P3` baseline proven machine-state, not code).
- **P4T in flight**: **T-1 DONE (T1a+T1b)**, **T-2 DONE** — importer
  golden file suite `crates/oxidizr-import/tests/golden_files.rs`: 9 tests,
  11 authored fixtures under `tests/fixtures/` (README + inventory test),
  `expect!` digests blessed; MADR fixtures carry front-matter `date:` (the
  importer's date fallback is file mtime — nondeterministic goldens
  otherwise; matrix T-2 cell now names `golden_files`). **T-3 DONE** —
  manifest schema v3 (`AxisReport` {exit, codes, one count}): `inspect`
  (violations @ threshold `info`), `query` (smoke filter
  `element.tag==Element`, kind All, JSON), `diff` (self-diff → findings 0)
  over the entry's OWN input + `generate` over the ENTRY DIRECTORY
  (P4T-V1 settled; sidecar never matches `DEFAULT_INCLUDE`, blessed
  `views: 0` pins the view-free merge); 307 sidecars re-blessed, re-bless
  all-`unchanged`; OXZ-5010 entered the corpus via load-failing entries —
  the 5010 allow-list entry REMOVED (stale-entry tripwire fired as
  designed); full tier adds Invariant 4
  `generate_pair_merge_is_view_free_and_id_unique`; coverage gate reads
  the v3 sections; matrix cells + sidecar prose stamped to v3. Axes run in
  the QUICK tier (measured cost kept the quartet light).
  **T-4 DONE** — `tests/transcript.rs` grew per-capability transcripts
  over the real `run` loop (`completion_transcript`, `hover_transcript`,
  `formatting_edit_idempotence_transcript` — messy→1 edit→apply→0 edits —
  and `fast_edit_burst_converges`: open + 8 queued didChanges, LAST
  publish = LAST version = LAST buffer's truth). P4T-V3 VERIFIED: the
  converge check is wall-clock-FREE (FIFO single-threaded loop; full
  exchange scripted before the drain; zero sleeps). Summarizer per-shape
  lines: completion `n [first 3 labels]`, `edits=n`, hover first markdown
  line. **T-5 DONE** — 4 fuzz targets (`import_sniff`/`query_parse`/
  `pathfilter`/`diff_pair`) + 28 seeds, `FUZZ_TARGETS` staged to 7; the
  battery found TWO REAL engine bugs, both fixed code-side: fmt BOM
  idempotence (`48aa18b`, protective line-START space — the BOM-after-indent
  is a TOKEN char the reference strips only at byte 0) and pathfilter
  zero-width doubling (`0e8d7f3`, `$+$+…` was 2^n — each continuation
  position tried exactly once, JDK-witnessed table; P4T-V2 VERIFIED: YES,
  real finds). Slow-unit backtracking = PARITY with `java.util.regex` →
  fixed ≤4-char fuzz subjects bound the surface, NEVER a matcher cap.
  Finds note: `testing/fuzz-p4t-finds.md`. **T-6 DONE** — CLI audit row:
  `export` was the ONLY `Command` without subprocess tests; 4 legs in
  `crates/oxidizr-cli/tests/cli.rs` (view+legend write loop, the two
  reference INFO notes byte-exact on stderr both exit 0, the OXZ-5007/3/2
  exit family — `static`/`png` exit 1, the reference AGREES for PNG/SVG —
  and the WSD dynamic-view leg); `Command::Export` doc comment refreshed
  (png/svg are KNOWN OXZ-5007 formats, not unknown exit-0) — `71609ad`.
  Gap 7 recorded as M5b-E-2-delivered (`export-verify --format
  c4\|plantuml\|mermaid` over the full golden dirs) and RUN at close.
  **P4T CLOSED 2026-11-22**: full e2e tier green · bless 307×unchanged +
  `testdata/` clean · `oracle verify` 4 json + 4 sidecar + 99 diagram +
  6 theme · `export-verify` 31/31 × 3 · `oracle landscape` 10 facts (all
  `TZ=UTC`) · PROGRESS board row + milestone log · matrix FROZEN (zero
  `T-n` cells) · **no tag** (interlude precedent).
- **§P4R R0 DONE 2026-11-22** (inventory wave, `build/PHASE_4R.md` D2–D4):
  allow audit 157 `allow(clippy::…)` sites / 88 files — non-test ~106,
  EVERY site justified (ratchet honest; real items left = three
  `struct_excessive_bools`: `ApiOptions` R3, cli flags R4, viewer shell
  R4); `cargo doc --workspace --no-deps` baseline **43 warnings → 0**
  (18 files, doc-comment-only: private links → code spans, fn/mod
  ambiguities → `fn@`/`mod@`, cross-module → `crate::` — ZERO is the
  drift detector for every later wave); bench baseline **`4T`** saved +
  committed (104 files, `cargo xtask bench cmp 4T` for hot-path waves);
  D2 catalog re-measured with 3 seed CORRECTIONS (generate.rs has NO
  HashMap ladders — `Vec` scans; `command_name` is `#[cfg(test)]`-only;
  `oxidizr-inspect` is 5 modules not ~35); wave table R1…R5 FILLED with
  real items; churn list skeleton `agents/notes/
  process/api-churn-v0.4.0.md` created (V3: E-5 folds it into
  CHANGELOG.md; first churn commit adds the first line).
- **§P4R R1 DONE 2026-11-22** (core/ast/diag/dsl/json wave):
  `create_default_views` → three named leg helpers, `too_many_lines`
  allow GONE (`2d098f5`); dsl `needless_pass_by_value` ×2 re-audited —
  both allows GONE: `register_identifier_element` consumes its owned
  `RegItem` (id read out before the move, clone saved),
  `add_perspective_to_holders` takes `&Perspective` (`55037ae`); rest =
  KEEP rows (arm-order `too_many_lines` ×7, Java-contract casts). Wave
  end: full e2e tier green · bless 307×unchanged · doc-zero held ·
  `cargo xtask bench cmp 4T` ALL 104 comparisons at 1.00. **Next: R2** —
  typed `ImportFormat` constructor absorbing the cli extension-sniff
  table (FIRST churn commit → add a line to `process/api-churn-v0.4.0.md`
  — API churn), `regexlite`/`pathfilter` keep-split decision. Then R3
  (`Request` per-domain rework flagship + `ApiOptions` flags struct),
  R4 (cli report plumbing, viewer shell bools), R5 (pub-use
  consolidation, churn list complete).
- **P4T/P4R/P4D scaffolded** (plan-only commits) — user decisions recorded
  in the three D0s: harden-before-release; 4R may churn the public API
  INCLUDING structural enum-shape reworks (bytes/CLI frozen, CHANGELOG
  line per API commit); provenance of the Java reference lives ONLY in
  `agents/plan/` after P4D.
- **Release endgame ratified 2026-11-22**: E-5 ends LOCAL + local `v0.4.0`
  tag (push/publish = user); crates.io publishing SKIPPED for v0.4.0;
  cargo-dist 0.34.0 added to the flake dev shell (E-4 verifies
  `cargo dist check` locally; CI authority unchanged).

## State

- **P1 COMPLETE** (tag v0.1.0), **P2 COMPLETE** (tag v0.2.0), **P2R
  COMPLETE** `410fee6`; **P3 COMPLETE 2026-11-20 — tagged `v0.3.0`
  (local)**:
  M0 `346ca6e`; M1 (`969a31b`→`496912c`); M2 (`c626048`→`468ec03`, `P3`
  bench baseline); M3 (`4fcb235`→`d39ebdc`); M4 (`81b0a3d`→`dc5d840`);
  M5: M5-0 `5f1dd32`, M5-1 `0842dbd`, M5-2 `2325947`, M5-3 `ff2b8b5`,
  close + tag. **Post-tag M6** (display auto-layout, §P3-M6): code
  `a1e7dfc`, docs `0083fe6`. **Post-tag M7** (§P3-M7): sidebar view list
  grouped per view kind, `e60dcba`. **Post-tag M8** (§P3-M8, F10):
  `oxidizr view` in-binary GUI subcommand — shell→lib `shell.rs`
  `55a0219`, cli entry `6bd7073`, gates/docs (this commit). **Post-tag
  M9** (§P3-M9): Windows CI reload fix — notify `Modify(Any)` class
  accepted on windows-cfg only + platform-mapping test. **Post-tag
  M10** (§P3-M10): display-layout separation floor (`DISPLAY_FALLBACK`
  200/120, graphviz lower-bound) + right-button pan. **Post-tag
  M11** (§P3-M11, twelve-item UX batch, 2026-11-20/21): C1 `c67d16b`
  (gesture swap + trackpad split + tooltip), C2 `33d4b0a` (waypoint
  delete + hover handles + drags carry routes), C3 `131588b` (side
  anchors), C4 `357d90c` (text-last paint), C5 `5ce2146` (search icons +
  path once), C6 `27957b5` (selection info panel), C7 `6d11861`+`6728166`
  (engine port spread + scene slide rule), C8 `f499fb7` (F11
  `<stem>.oxidizr.json` sidecar + viewport memory). Milestone closed:
  full e2e tier + bless 307×unchanged.
  **Nothing pushed; tags v0.1.0/v0.2.0/v0.3.0.**
- **Ritual:** quartet every commit; shell commits also `cargo xtask ci
  gui`; lib commits also full tier (`OXIDZR_E2E_FULL=1 cargo test
  -p oxidizr-e2e` → 8 ok) + `cargo xtask e2e bless` → 307×unchanged +
  clean `testdata/`; commit every green sub-piece; **never push**.

## Next: Phase 4 (`build/PHASE_4.md` — authoritative, read it first)

- **§P4-LSP CLOSED 2026-11-21** (F5 prioritized per user directive):
  L0 `a38f905`, L1 `e11271f`, L2 `fedd7aa`, L3 `e13af48`, L4 `16d9ab5`,
  L5 `a950748` — full e2e tier green + bless 307×unchanged +
  `cargo xtask ci` green; close-out notes in PHASE_4.md (L-7 DONE row)
  and PROGRESS.md (status board + `P4-LSP` milestone log).
- **§P4-M0 `diff` CLOSED 2026-11-21** (F3; D-1…D-4, commits M0a `c121cbe`,
  M0b `85b3297`, M0c `4c7612e`, M0d this): engine `oxidizr-api/src/diff.rs`
  (identity by canonical name, `idChanged`, `ignore_layout` gate) + schema
  1 (`docs/diff-schema.md`) + pipeline/CLI (`oxidizr diff A B [--format
  human|json] [--ignore-layout] [--quiet]`, exit 1) + fixtures
  `testdata/probes/diff/*` + `crates/oxidizr-api/tests/diff_probes.rs`
  (snapshots + corpus sweeps) + `Tags::effective` in core. Full e2e tier
  green + bless 307×unchanged + `cargo xtask ci` green; stamps: PHASE_4
  §P4-M0 CLOSED row, PROGRESS status board + `P4-M0` milestone log.
- **§P4-M1 `inspect` CLOSED 2026-11-21** (F-inspect-parity; I-1…I-4,
  commits M1a `cad75b9`, M1b `9e7b599`, M1c `9bf088b`, M1d this): new crate
  `oxidizr-inspect` (pure model logic over core, ADVISORY — P4-D7), the
  `DefaultInspector` port (workspace+model+view phases, run order +
  messages VERBATIM, P4-V2 severity strategy), `oxidizr inspect <WS>
  [--severity-threshold info|warning|error] [--only T]… [--except T]…
  [--format human|json] [--quiet]` + schema-1 doc (`docs/inspect-schema.md`),
  exit 1 iff REPORTED. Full e2e tier green + bless 307×unchanged +
  `cargo xtask ci` green; stamps: PHASE_4 §P4-M1 CLOSED header (I-rows all
  DONE), PROGRESS status board + `P4-M1` milestone log. Surface notes below.
- **§P4-M2 importers CLOSED 2026-11-21** (N-1…N-6; commits M2a
  `b7f66e5`, M2b `888bdf8`, M2c `97e3bcb`, M2d `1ea06c8`, M2e `e753d4e`,
  M2f this): the `oxidizr-import` crate (image + C4-PlantUML + Mermaid
  ports — F12 semantic bar, MODEL ONLY; surface notes in the
  §P4-M2 rows of `build/PHASE_4.md`), the WSD exporter (oracle 9/9
  byte-parity, `golden/websequencediagrams/`), and N-6/M2f =
  `!docs`/`!adrs` engine wiring — `docs.rs` (`DefaultDocumentation-
  Importer`/`FormatFinder` + recursive walk + the reluctant image pass),
  `decisions.rs` (AdrTools/Madr/Log4brains family, UTC parsing = D15),
  `regexlite.rs` (std-only Java-regex FULL-MATCH subset, P4-V8), engine
  `run_import` (six-FQN registry, `loadClass` first, empty-exclude guard,
  reluctant pass after ANY importer when the path is a directory);
  OXZ-5003 retired to a tombstone (m4 register 69 clean / 35 registered;
  e2e bless = 305×unchanged + the 2 reviewed sidecars: `04-deferred-
  directives` 5002+5003→2002+5002, `15-adrs-dir` 5003→2002); oracle
  A/B legs green `TZ=UTC` (see §P4-M2 surface below, esp. the
  Java empty-input `split` fix). Surface notes below; permanent contract
  `build/IMPORT-DOCS-ADRS.md`. Close stamps: PHASE_4 N-6 + Close rows,
  PROGRESS status board + `P4-M2` milestone log.
- **§P4-M3 O-1 `query` DONE 2026-11-21** (M3a, F6): `oxidizr query <WS>
  --filter EXPR [--kind element|relationship|view|all] [--format
  table|ids|json] [-o]` — the DSL item-expression grammar IS the query
  language (P4-D3 corrected: `oxidizr_dsl::query::run(&Workspace, expr)`
  public entry = engine reuse via `adopt` + the extracted
  `register_json_identifiers`; P4-V4 CLOSED, token table in
  `docs/query-schema.md`). Surface note below.
 - **§P4-M3 O-2 `init` DONE 2026-11-21** (M3b, F7, P4-D4): `oxidizr init
   [DIR] [--template minimal|c4] [--force]` — `include_str!` scaffold
   (zero network, zero new deps), idempotent, exit 0 even all-skipped;
   templates stored CANONICAL (pass `fmt --check` — the printer's
   conservative unquoting is the required spelling); CI teeth = scaffold
   validates + masked `dsl2json` expect! golden per template. Surface
   note below.
 - **§P4-M3 O-3 `completions` DONE 2026-11-21** (M3c, P4-D10): `oxidizr
   completions <bash|elvish|fish|powershell|zsh>` via `clap_complete`
   (the ONE new dep, workspace table) — dispatched in `run` before the
   pipeline like `view` (renders the argv table itself, never a
   `Request`; script = stdout payload, exit 0; unknown shell = clap 2).
 - **§P4-M3 O-4 `generate system-landscape` DONE 2026-11-21** (M3d,
   P4-D9): `oxidizr generate system-landscape <1..N paths> [--include
   R] [--exclude R] [--relationships first|all] [-o]` — the reference
   `SystemLandscapeGenerator` port in `oxidizr-api/src/generate.rs`
   (oracle witness: adds NO views; contract in the PHASE_4 O-4 DONE
   stamp + surface note below). New OXZ-5010 `GENERATE` for filter
   patterns outside the std-only subset. `cargo xtask oracle landscape`
   = the Category B leg (10 facts match).
 - **§P4-M3 CLOSED 2026-11-21** (O-1…O-4, commits M3a `da0ff38`, M3b
   `905ef9f`, M3c `4224cb0`, M3d `579d2aa`, close this): close ritual =
   `cargo xtask ci` green + bless 307×unchanged (clean `testdata/`) +
   full e2e tier green + PHASE_4 CLOSED header/Close row + PROGRESS
   status board + `P4-M3` milestone log.
   **Next: §P4-M5 — F4 `oracle` user command, P4-D13 carry-ins,
   cargo-dist 3-OS + tag `v0.4.0` (local), changelog catch-up, P4-V6.**

## §P4-M3 O-4 generate surface (`oxidizr-api/src/generate.rs` + `pipeline::generate_cmd` — read before touching)

- **Merge contract (witnessed 2026-11-21, TZ=UTC legs)**: name-first
  person/system clone; description AND url set UNCONDITIONALLY per
  workspace (LAST wins, nulls nuke); group first-wins; stored-tags union
  via `Tags::add`; properties/perspectives merge; one shared id counter
  (elements then relationships); `{workspace:<id>}/diagrams` onto the
  FIRST container-bearing system per workspace; `findElement` is
  people-first (a same-named person shadows a system). `First` keeps
  one edge per source→destination pair, `All` one per distinct
  description (the `hasEfferentRelationshipWith` twin).
- **`pathfilter` is a FULL-match std-only engine** (the reference =
  `Pattern.matches` over the FULL path incl. directory components).
  Supports literals, `.`, `\d \D \w \W \s \S`, `[...]` classes (shorthands
  ONLY as escapes — `[d]` is the letter), groups, `|`, `* + ?` (lazy
  spellings parse, greediness is irrelevant to a boolean match), `^`/`$`.
  `{m,n}`/lookaround/`\p{…}`/named groups/inline flags ⇒ OXZ-5010 exit 1
  — NEVER widen silently. Engine gotcha: `star()` must try the
  zero-iteration continuation at its own position or `.*x` cannot match
  text starting at `x`.
- **Exit family**: 0 ok · 1 zero workspaces ("Two or more workspaces
  must be provided" verbatim — ONE workspace is LEGAL, witnessed) or
  bad pattern · 2 clap usage · 3 missing path. Explicit file
  positionals ALWAYS load (documented deviation); load failures are
  per-file `Could not parse …` notes; output ALWAYS JSON.

## §P4-M3 O-2 init surface (`oxidizr-api/src/init.rs` + `templates/` — read before touching)

- **Templates are canonical DSL bytes**: `templates/{minimal,c4}/workspace.dsl`
  must pass `validate` AND `fmt --check` (drift breaks CI — intended).
  Adopt the PRINTER output, never hand-quote: `person User`,
  `systemLandscape landscape` are the canonical spellings. Grammar fact
  that failed the first draft: view headers take `[identifier] [key]
  [description]` ONLY — titles via in-view `title`/`description`
  statements.
- **Report bytes are the contract** (CLI + probe goldens pin them):
  five `created <rel>\n` lines in template order (workspace.dsl,
  README.md, .gitignore, .editorconfig, .vscode/settings.json), skip
  variant `skipped <rel> (exists, use --force to overwrite)`, then the
  `next steps:` footer. Exit 0 even when everything is skipped; Io
  failure = exit 3 with the first `Issue::io`; nested dirs created.
- **Golden mask**: `init_probes.rs` strips every `structurizr.*` key
  from `properties` objects (loader-regenerated UUIDs/base64 per parse —
  the oracle-protocol class) before the expect! compare.

## §P4-M3 O-1 query surface (`oxidizr-dsl/src/query.rs` + `oxidizr-api/src/query.rs` — read before touching)

- **One parser, one evaluator**: `oxidizr_dsl::query::run` = the pinned
  `ExpressionParser` port on `ExprScope::Model` (`adopt` + JSON-identifier
  register; `Parser::new` is `pub(crate)`); Model-scope unmatched ⇒ EMPTY,
  never throws — `query_cmd` maps a THROWING arm to Diagnostics with the
  reference message (NO new diagnostic codes; zero matches exits 0 —
  informational). Grammar facts (test-proven): values are BARE tokens;
  `element.type==` takes the enum spelling (`SoftwareSystem`); no `!=`
  forms for type/group/properties; bare `->` throws.
- **View matcher mirrors `parseExpression` EXACTLY**: `" && "` checked
  BEFORE `" || "`; split halves go to the SINGLE-arm evaluator (no
  recursion — a compound half compares its whole remainder as the literal
  value); Java `split` `[0]`/`[1]` ⇒ a THIRD arm is DROPPED. Subset =
  `key|title|description ==|!= value` (arm prefix case-insensitive, values
  case-sensitive `equals`); views have NO tags; outside-subset arms match
  NOTHING (the base-parser `default:` fail-soft).
- **Report layer** (`oxidizr-api/src/query.rs`, pure): rows via `adopt` —
  canonical names + EFFECTIVE tags (diff D-4 machinery); `--kind view`
  skips model evaluation; formats table/ids/schema-1 (`docs/query-schema.md`);
  JSON sections follow `--kind`, engine result order, key order frozen.
- Tests: 5 DSL + 5 API unit, `query_probes.rs` 6 (twin-identical over 15
  filters, corpus never-panics — open-stage Io (`iso-8859.dsl`) skipped),
  4 CLI goldens. `to_request` gained `#[allow(too_many_lines)]` (the
  argv→Request table precedent, like diff.rs field matrix).

## §P4-M2 importers surface (`oxidizr-import` + engine `run_import` — read before touching) **Next: §P4-M3**
  (`query`/`init`/`completions`/`generate` per PHASE_4), then M5 (oracle
  legs, carry-ins, cargo-dist, changelog catch-up).
- Scope: **F3 `diff`, F6 `query`, F7 `init`, F5 `lsp`, F4 `oracle` user
  command, inspect-parity**, diagram importers moved from P2
  (C4-PlantUML, Mermaid, image→CustomView), WebSequenceDiagrams
  exporter, release engineering (cargo-dist 3-OS). New crates per
  P4-D1: `oxidizr-import`/`-lsp`/`-inspect`; the rest are `oxidizr-cli`
  subcommands. diff exit-code adds `1 = differences found`.
- **Nothing may alter `oxidizr-json` bytes** (P4 opening clause) —
  bless 307×unchanged stays the tripwire.

## §P4-M2 importers surface (`oxidizr-import` + engine `run_import` — read before touching)

- **Two fidelity bars**: macro importers (F12) = SEMANTIC, model only,
  goldens are OURS; `!docs`/`!adrs` = core-compat — the byte contract
  where the format defines it. Permanent port contract:
  `build/IMPORT-DOCS-ADRS.md` (pinned facts, error shapes, oracle
  protocol). Deviations D15 (decision-date PARSING is UTC; oracle legs
  run `TZ=UTC`) + D16 (docs `Image.content` = raw file bytes base64'd,
  NO data-URI prefix).
- **Engine `run_import` order** (the `end()` contract): skip if no
  `current_file_raw()` (string parses import NOTHING) ⇒ resolve FQN
  (registry = the six built-in FQNs; unknown ⇒ `"{prefix}{fqn} was not
  found"` — `loadClass` BEFORE any path/regex error) ⇒ empty-exclude
  guard (`"A regex must be provided."`) ⇒ `new File(parent, path)` join
  (no `..` resolution) ⇒ import ⇒ reluctant `DefaultImageImporter(true)`
  pass (BOTH contexts, after ANY importer, ONLY when the joined path is a
  directory). `ImportError::Failed(m)` ⇒ prefix+m; `Io` ⇒
  `"java.io.IOException: {e}"`. Ctx payloads carry the block-end
  line/token ⇒ OXZ-2002 spans match the JVM text byte-for-byte.
- **`exclude` = Java regex FULL MATCH** (P4-V8: `equals || matches`) ⇒
  std-only `regexlite` (literals, `.`≠newline, `\d\D\w\W\s\S`,
  classes, groups/alternation, greedy+lazy `* + ? {n,m}`; NO
  `(?flags)`/`\b`/backrefs — unreachable in the corpus). `!adrs`
  DEFAULT = adrtools; aliases case-sensitive `getOrDefault`; `end()`
  dispatches by resolved CLASS (a `!docs` naming a decision importer
  imports decisions).
- **Java `split` semantics (compat bug caught by the oracle A/B and
  fixed)**: `"".split(sep)` → `[""]` (ONE empty element — the
  documented empty-input special case, JVM-proven on the pinned Java 21);
  non-empty all-empty input stays `[]` (`",".split(",")` → `[]`). The
  element/deployment-node parsers `addAll(token.split(","))` the TAGS_INDEX
  token unfiltered, so `deploymentNode "X" "" "tech" ""` MUST store the
  empty tag ⇒ `"tags": "Element,Deployment Node,"`. Fixed centrally in
  `java_split_str`/`split_keeping_lead_dropping_trail` (`engine/mod.rs`);
  the expression-split arms are `contains`-guarded (empty input
  unreachable) and `java_index_or_oob` keeps its non-empty-`[]` message.
- **Oracle A/B protocol** (`TZ=UTC`, app 2026.10.26): `oxidizr dsl2json`
  vs reference `export --workspace X.dsl`, compare via `jq -S`; mask the
  TWO established classes — `structurizr.dsl.identifier` UUIDs are
  RANDOM per-parse on BOTH sides, and the reference CLI injects
  `structurizr.inspection.*` counters into top-level `properties` at
  export (strip; if it empties, delete the key — we never write it).
  Read gate = reference re-exports OUR JSON (its JSON `setTags` drops a
  trailing empty tag on re-import — the reference's OWN asymmetry, so
  mask `"\u2026Node,"` → `"\u2026Node"` there). Negative legs: the
  deferred fixtures' error text matches verbatim incl. line + token.
- Tests: `oxidizr-import` 55 lib; `crates/oxidizr-dsl/tests/
  p4m2_docs_adrs.rs` 5 (engine wiring incl. both negative arms); m4
  register `m4_fixtures.rs` 69 clean / 35 registered.

## §P4-LSP shipped surface (oxidizr-lsp + engine/scope.rs — read before touching)

- **Crate ritual** (R-2 mirror): `oxidizr-lsp` = headless lib core
  (`server.rs` state machine + `diagnostics.rs`/`completion.rs` pure
  fns) + thin stdio `main.rs`; `run()` takes `Connection` BY VALUE and
  destructures it — dropping `sender` ends lsp-server's writer thread
  (joining io-threads while holding the connection HANGS). Transcript
  harness (`tests/transcript.rs`) pins wire ORDER via
  `Connection::memory()`; unit tests pin CONTENT.
- **Diagnostics (L1)**: `.json` URIs route to `oxidizr_json::from_slice`
  (`Input::Bytes` has no extension); DSL = `api::open(Input::Bytes)`;
  UTF-16 column mapper `diagnostics::position`; cap 100; `didClose`
  ends with an EMPTY publish.
- **Completion (L2)**: `oxidizr_dsl::engine::cursor_scope(source,
  cursor_byte)` replays the REAL engine over the lines BEFORE the
  cursor (`Logical.span` is **(start, end)** — the guard is
  `span.1 > cursor ⇒ break`; the cursor's own line is NEVER fed);
  recovery mirrored (`stack.truncate(height)`, `fatal_file ⇒ stop`);
  `ParseOptions::fragment()` + CLOSED `env_lookup` ⇒ FS-free and
  deterministic. Returns purpose-shaped `CursorScope { keywords,
  expects_workspace, identifiers, view_keys, tags }` — `Ctx`/`Parser`
  stay `pub(crate)` (2R R-dsl-09). `DIRECTIVES` excludes the JVM-only
  `!plugin`/`!script`. `IdentifiersRegister::all_identifiers()` drops
  auto-UUID keys.
- **Completion wire** (`completion.rs`): keywords→Keyword,
  directives→Keyword, identifiers→Variable, view keys→Property,
  tags→Field, snippets→Snippet (`per`/`sys`/`rel`; `rel` gates on
  `->` — the Model scope has no `relationship` keyword); `!`-prefix leg
  returns directives ONLY; prefix walker set `[A-Za-z0-9_!-]`;
  `position_to_offset` clamps both ways (this nightly: `len_utf16()`
  returns `usize`). Server: unknown doc ⇒ EMPTY array (not an error),
  malformed params ⇒ `InvalidParams`; capability triggerCharacters
  `["!"]`.
- **Engine facts pinned by probes**: `systemLandscape "key1" "L"`
  OPENS a view context even without `{` (scope-test fixtures must brace
  view blocks); the CLOSED root still lists `[name, description, …,
  model, views, configuration]` — `expects_workspace` is what marks a
  fresh file, not an empty keyword list.
- **Hover (L3)** (`hover.rs` + `server.rs`): token under the cursor =
  quoted string STRICTLY containing it (unescaped-quote pairing from the
  line start; an unclosed quote hovers as opened), else word run
  (`[A-Za-z0-9_]`, a leading `!` joins). Legs: model card beats keyword
  card; identifier match (`structurizr.dsl.identifier` property the
  parser records) beats name match; directives compare case-free,
  keywords exact. `Server::last_good: HashMap<Uri, Workspace>` —
  replaced ONLY by a clean pipeline run (`diagnostics::workspace`:
  `api::open` ok AND log carries NO `LogItem::Issue`; JSON URIs:
  `from_slice` ok), kept across failing reparses (broken buffer hovers
  the model as last parsed), dropped on `didClose`. Wire: unopened doc /
  nothing hoverable ⇒ JSON null; malformed params ⇒ `InvalidParams`;
  capability `hover_provider = Simple(true)`.
- **DSL facts pinned by hover fixtures** (each produced a real parse
  error first): positional strings are name + DESCRIPTION (technology is
  the block statement); a bare `deploymentNode` in `model` is INVALID —
  needs a `deploymentEnvironment` wrapper (OXZ-2001 expected list);
  `infrastructureNode` is positional-only `<name> [desc] [tech]` — opens
  NO block (OXZ-2002); qualified `s.c` identifier refs need
  `!identifiers hierarchical` (corpus: deployment-groups-hierarchical.dsl);
  `containerInstance` form is `containerInstance <id>` /
  `<system-ident>.<container-ident>` — NEVER `s:c`.
- **Formatting (L4)** (`formatting.rs` + `server.rs`): `formatting` =
  ONE whole-buffer TextEdit from `oxidizr_dsl::fmt::format`, EMPTY array
  when canonical OR `.json` (byte contract never sees the DSL printer —
  `diagnostics::is_json` is the shared `pub(crate)` route);
  `rangeFormatting` WIDENS to the top-level statement(s) touched
  (canonical style is a global re-print — mid-statement edits would
  disagree with the next full format; documented deviation); cursor in
  top-level trivia ⇒ NEXT statement, trailing ⇒ PREVIOUS; statement
  walk over the lossless CST with brace delta from `TokenBare` ONLY
  (quoted values/`"""`/comments never move depth), stray closers clamp
  at 0, open braces at EOF end the statement at buffer end; statement
  spans end on a line-terminator boundary (`terminator_end`);
  `FormattingOptions` read but NOT honored (canonical = 4-space);
  unopened doc ⇒ EMPTY array, malformed params ⇒ `InvalidParams`.
  Formatter quirk pinned by tests: conservative unquoting — `name "x"`
  prints `name x`.
- **Config (L5)** (`config.rs` + `server.rs` + `diagnostics.rs`):
  `initializationOptions` `{strict, themesDir, allowNet=false,
  maxNumberOfProblems}` parsed by a HAND-ROLLED total read over
  `serde_json::Value` (no serde-derive dep): `Config::from_options(
  Option<&Value>)` reads each key independently, wrong-typed or unknown
  keys are IGNORED, a non-object value is "no options" — configuration
  NEVER fails `initialize`. Manual `Default` (derived would zero
  `max_problems`; the default is `MAX_DIAGNOSTICS` = 100; `0` is honored
  literally, negatives/fractions via `as_u64` fall to the default).
  `Server::config` is captured in `initialize_response` (now `&mut
  self`, params parsed `ok()`-style); `diagnostics::collect`/`workspace`
  take `&Config` so `api_options()` (`strict`/`themes_dir`/`allow_net` →
  `ApiOptions`, D5: no network without `allowNet`) rides EVERY pipeline
  run and `max_problems` caps every publish. `--version`/`-V` pinned by
  `tests/version.rs` through `CARGO_BIN_EXE_oxidizr-lsp` (own-package
  bin — no e2e-style binary walk needed).
- lsp-types 0.97: `completion_provider` IS `CompletionOptions` (no
  provider wrapper); `CompletionResponse::Array`; `Position::new`;
  `HoverRequest` (NOT `Hover`) is the request type — METHOD
  `textDocument/hover`, Result `Option<Hover>`, `HoverParams` field is
  `text_document_position_params` (flattened, differs from completion's
  `text_document_position`); `HoverProviderCapability::Simple(bool)`;
  `Formatting`/`RangeFormatting` ARE the request traits (METHODs
  `textDocument/{,range}Formatting`, Result `Option<Vec<TextEdit>>`,
  `DocumentFormattingParams{text_document, options}` + `range` on the
  range variant); the two capabilities are
  `Option<OneOf<bool, …Options>>` — `OneOf::Left(true)` serializes to
  `true`; `FormattingOptions` JSON REQUIRES `tabSize`+`insertSpaces`.
  `len_utf16()`/`len_utf8()` return `usize` on the pinned nightly;
  `usize: From<u32>` does NOT exist — u32 spans go through
  `usize::try_from(..)` or `as usize` (cst.rs style).

## §P4-M0 diff surface (`oxidizr-api/src/diff.rs` + pipeline + CLI — read before touching)

- **Identity** (P4-D2): elements keyed by `canonical_name_full` via
  `WorkspaceBuilder::adopt` (the `Person://` URI prefix IS the type),
  relationships by `relationship_canonical_name`, views by key, styles by
  (kind, tag). SAME identity + drifted id ⇒ `idChanged`, NEVER add+remove;
  element NAME / relationship DESCRIPTION changes ARE identity changes
  (embedded in the key ⇒ add+remove pairs); duplicate canonical names pair
  positionally (`#N`); dangling refs ⇒ `Unresolved://<id>` fallback keys.
- **THE THREE REPRESENTATION TRAPS** (each broke a smoke/probe BEFORE it
  could ship; all pinned by tests): (1) `structurizr.dsl*` properties are
  loader-REGENERATED per parse (fresh UUID per item + base64 source
  snapshot) — NEVER compared (`push_props` skips the prefix); (2) loaders
  represent tags differently — the DSL path keeps kind defaults DERIVED, a
  JSON load re-stores the CSV (reference `setTags`) — so tags compare as
  the reference EFFECTIVE view `getTags()` = defaults ++ stored, deduped
  (new `oxidizr_core::tags::Tags::effective`; `base_fields` takes the
  `TagKind`, `rel_fields` computes `relationship_defaults(linked, style)`);
  (3) the DSL parser defaults `description` to `""` while the JSON writer
  OMITS empty text — `opt()` reads empty strings as ABSENT. Consequence:
  a `.dsl` and its JSON twin diff EMPTY (`a_dsl_and_its_json_twin_diff_empty`).
- **Pipeline/CLI**: `Request::Diff { a, b, json, ignore_layout, quiet }` →
  `diff_cmd` (both sides via `api::open`; partial DSL opens ride the log
  informationally); `ExitClass::Differences` = code 1 (the documented P4-D1
  extension; the e2e `Exit::from_class` folds it onto `error`, the corpus
  never emits it); `ApiOptions.color` — CLI fills `!no_color && NO_COLOR
  unset`, API/e2e stay PLAIN. `--quiet` = exit code ONLY (no bytes, no
  chatter). CLI flag spelling: `#[arg(long = "ignore-layout")]` REQUIRED
  (the `Command` lowercase rename would flatten it to `--ignorelayout`).
- **Tests**: 13 unit tests in `diff.rs`; `tests/diff_probes.rs` (6): the
  pair snapshots (`expect!`, re-bless `UPDATE_EXPECT=1 cargo test -p
  oxidizr-api --test diff_probes`), the two-load determinism sweeps
  (`testdata/dsl` unopenables SKIPPED, floor assert; `testdata/validation/*`
  EXCLUDED — deliberately invalid fixtures self-pair non-empty ABOVE the
  `Unresolved://` fallback by design); CLI goldens in `oxidizr-cli`
  (`diff_*`, temp fixtures, `NO_COLOR=1`): human headers `added (N):`,
  lines `+ path (kind)` / `~ path field: <json> -> <json>`, silent-when-
  equal exit 0, differences exit 1, missing side exit 3.
- **Fixtures** `testdata/probes/diff/*`: `base.dsl`/`edited.dsl` hand-curated
  edit story + `base.json`/`edited.json` = `dsl2json` twins (README
  contract row; NOT oracle-witnessed — Structurizr has no `diff`). The
  story covers every category: add/remove, the relationship-description
  rename pair, field/style changes, membership shift, id drift.

## §P4-M1 inspect surface (`oxidizr-inspect` + pipeline + CLI — read before touching)

- **Engine** (`run.rs` = the `DefaultInspector` port): `inspect(&Workspace)`
  runs workspace → model → view phases and returns violations in EXECUTION
  order (never sorted); `Violation.toString()` = `type | SEVERITY |
  message` with Java enum names. Messages VERBATIM from the pinned Java —
  pinned quirks: `Model.isEmpty()` counts people+systems+deploymentNodes
  ONLY (custom elements never count); the noview message alone uses
  `getName()`, every other one the canonical name minus `Type://`
  (`ModelItem::name_of`); filtered/image views get the generated-key
  inspection ONLY (no content ⇒ no empty/layout trio); styles fire ONLY on
  `metadata == Some(false)`.
- **Severity strategy** (`severity.rs`, P4-V2 CLOSED from pinned Java):
  property key `structurizr.inspection.<lowercased dotted type>` —
  SINGULAR `inspection`; longest-first `*` wildcard fallbacks;
  relationship types rewrite to `.relationship[<srcType>-><dstType>].`
  (source type first); holder chains inner→outer FIRST-HIT-WINS
  (element→own props→…→workspace; view→view-config→workspace; style→
  view-config→workspace); NO match ⇒ ERROR; `structurizr.inspection=
  false` kills the run. Deviation (registered): unparseable severity
  VALUES map to ERROR (Java throws).
- **Filters/reporting** (`inspect_cmd` in pipeline): the engine keeps
  IGNORE violations (reference counters) — ONLY the CLI drops them;
  threshold keeps rank `error>warning>info`; `--only` (when set) BEFORE
  `--except`, exact type match; exit 1 iff REPORTED (`ExitClass::
  Differences`); human color paints ONLY the severity token
  (31/33/36 — goldens run `NO_COLOR` ⇒ PLAIN bytes are the contract).
- **Faithful CLI quirk**: `oxidizr inspect` loads via `api::open`, which
  installs default views whose keys ARE auto-generated — the goldens
  REPORT `views.view.key` + `views.view.layout` for
  `SystemLandscape-001`/`SystemContext-001` (pinned `createDefaultViews`).
- **Tests**: 15 lib + 19 integration (`inspect_model.rs`/`inspect_views.rs`
  snapshots re-bless via `UPDATE_EXPECT=1 cargo test -p oxidizr-inspect`;
  `inspect_complete.rs` proptest; `inspect_corpus.rs` sweep — lossy-read
  for `testdata/dsl/iso-8859.dsl`, network-include fixtures parse-fail and
  are skipped ⇒ hermetic). 8 CLI goldens `inspect_*` in
  `crates/oxidizr-cli/tests/cli.rs`.

## §P3-M11 UX batch surface (shell/paint/scene/layered/statefile — read before touching)

- **Paint (C4)**: `paint_scene` = background + GEOMETRY lane (reference
  order: boundaries→items→edges→selection) + ALL `Shape::Text` LAST
  (`split_text` partitions each layer). Invariant test
  `text_shapes_are_always_the_final_run`.
- **Arrow ports (C3+C7)**: engine `layered.rs` `pick_side` (shared by
  `endpoint` + the `port_spread` prepass; exact `endpoint(None)` rule:
  pinned anchor else dominant delta, ties→horizontal) fans a (node,
  side) group into N+1 slots, `along=(rank+1)/(N+1)`, ordered by far
  box-centre coordinate PARALLEL to the side (ties→input order). N=1
  keeps 0.5 — lone edges byte-identical. Scene `border_anchor`:
  centre→next RAY picks the side, anchor then SLIDES to the next
  point's facing cross coordinate CLAMPED to the side — engine legs
  share their port's cross coordinate, so stored routes re-anchor
  EXACTLY and every leg is axis-parallel (both exact-placement scene
  tests rely on it; the old 1e-9 tolerance died with the ray-parameter).
- **Sidecar (C8, F11)**: `statefile.rs` = `<stem>.oxidizr.json`
  (STEM naming ⇒ big.dsl/big.json pair shares ONE file; can never equal
  the contract sidecar `big.json`). Envelope `{"format":
  "oxidizr-viewer-state","version":1,"views":{k:{zoom,x,y}}}` via
  serde_json VALUES (untyped, BTreeMap order); atomic `{file}.{pid}.tmp`
  + rename; refuses to clobber a NEWER-version envelope; anything
  unreadable/wrong-shape loads EMPTY. NEVER in `save_target`, the
  reload candidate lists, or the watch set (exact-path filter drops its
  events — sibling test pins that). Shell: `adopt()` = flush old file
  (`record_view`+`flush_sidecar`) → seed `self.sidecar` from the new
  file → `pending_viewport` restores the camera on the FIRST painted
  frame (wins over `restore_scale` — cleared on apply — and over
  `needs_fit`, which adopt skips while pending). `record_view` stores
  zoom + WORLD centre from `last_rect` (resolution-independent).
  `reload_from_disk` pre-seeds `pending_view` = current view (adopt
  else falls back to the FIRST view). `eframe::App::save()` records +
  flushes (close hook); KEY_ZOOM stays as cross-file fallback.

## M5 hardening surface (xtask + probe — read before touching oracle verify)

- `cmd_oracle_verify` legs: json → **sidecar** → diagram → theme;
  summary `4 json + 4 sidecar + 90 diagram + 6 theme`. Sidecar leg
  (`verify_sidecar_legs` + `sidecar_edited_leg` + `collect_xy_maps`):
  open via `oxidizr_api`, save via `commands::write_workspace`;
  IDENTITY = reference re-export of our sidecar == re-export of the
  committed golden (both through `normalize_reference_output` —
  comparing RE-EXPORTS isolates our writer from reference quirks);
  EDITED = `layout::apply` → `apply_overlay` → save → reference load:
  `model` Value unchanged, id→x/y map survives verbatim AND differs
  from stored. xtask deps: oxidizr-api/core/viewer + serde_json
  (dep-table changes ⇒ stage `Cargo.lock` with the commit).
- Perf probe `crates/oxidizr-viewer/tests/perf_scene.rs` (ignored):
  `cargo test --release -p oxidizr-viewer --test perf_scene -- --ignored
  --nocapture`. Figures in `notes/perf/scene-paint-budgets.md` — 209
  items + 247 edges ≈ 1.0 ms/frame (~16× headroom); INFORMATIONAL
  (P3-D0), never a golden.

## M4-6 dot-backend surface (dot.rs + layout.rs — read before touching)

- `oxidizr-layout` feature `dot-backend` (OFF by default, adds only
  `std::process`): `Dot::from_env()` = `$OXIDZR_DOT_BIN` (empty ⇒
  None, oracle posture); `dot_source()` = input order, positional
  names `n0..` (ids NEVER enter the source), graph attrs
  rankdir/nodesep/ranksep/esep (÷ 72 px/in), splines from
  `generate_vertices`, tailport/headport for anchors.
- **`-Tplain` reality (graphviz 15.1.1, verified 2026-11-20 — NOT the
  man page):** node rows carry FULL extents (top-left = `(x-w/2,
  graph_h-y-h/2)`), edge points are TWO plain numbers (docs' joined
  `x,y` also parsed), the FIRST `graph` block starts at the origin
  (gap only BETWEEN components, 50 px), y is bottom-up, inches.
- `parse_plain` maps back in input order; parallel edges FIFO per
  (src,dst) pair. Category B test `real_dot_binary…` gated on the env
  var; determinism = fixed graphviz build.
- Viewer: feature passthrough `dot-backend`; `layout::Engine`
  (Layered always; Dot only feature + env) — `available()` powers the
  dialog combo (appears iff > 1 engine), `apply_with()` is the one
  translation pipeline (`apply()` = Layered, headless tests
  unchanged); Dot w/o env ⇒ `LayoutError::DotBackend` value. Engine
  choice SESSION-ONLY — `encode`/`decode` stay `"LR|100|50|50|1"`.

## M4-4/M4-5 shipped surface (overview.rs + export.rs)

- `overview::Overview::new(Scene::bounds, ERect) -> Option`: uniform
  fit + centred; `shapes(scene, view_world, selection)`; `map_to_world`
  inverse drives `Drag::Minimap` recenter. Quick-nav `nav_matches`
  feeds the left-panel search; field `text_focus` ORs toolbar + nav
  focus; guards Ctrl+Z/Y/S.
- `export::compose(scene, &mut FontsView) -> Composed{shapes,size}`:
  fitted `Viewport` (1:1 → 4096 cap, floor 0.25, 80 MP; degenerate ⇒
  400×240; selection NEVER painted); key band composed at ORIGIN then
  `Shape::translate(0, fig_h + KEY_TOP_PAD)` — never forget the
  translate. `shapes_to_svg` covers the closed Shape subset; text =
  per-glyph outlines via `ab_glyph` over the bundled Ubuntu-Light
  (workspace pin names `default_fonts` — READ
  `pitfalls/epaint-037-font-stack.md` before touching fonts/epaint).
  `svg_to_png` = usvg → resvg → `encode_png`; gate = parse OK + PNG
  magic (bytes NOT goldens, P3-D0). Shell: toolbar Export ⇒
  `<stem>.svg/.png` beside `commands::save_target` via
  `commands::write_atomic` (shared with save; `.{pid}.tmp`+rename
  never matches the watch filter). Notes: success `exported …`,
  fail/blocked `export …` (dark-red prefix set: `serialize:`,
  `write `, `export `).

## Reload stack (M4-1..M4-3 — read before touching reload)

- `ParseResult.includes` = FS `!include` files in parse order;
  `Opened.includes` carries it (empty for JSON/stdin).
- `reload.rs::ReloadDriver` (headless, notify, 250 ms settle-debounce,
  parent-dir NonRecursive + exact-path filter, SipHash baselines are
  the authority — `interesting()` is a PRE-filter only; on Windows the
  backend never emits `Modify(Data)`, plain writes are `Modify(Any)`, so
  that class is accepted under `#[cfg(target_os = "windows")]` (§P3-M9,
  pitfalls/notify-windows-modify-any.md). `Wake = Arc<dyn Fn() + Send +
  Sync>` (eframe 0.37 has NO RepaintSignal; shell passes `move ||
  ctx.request_repaint()`, `repaint_ctx` stored first frame). R-2 guard
  scans doc literals.
- Shell: poll hook + re-arm guard in `App::ui`; `adopt` drops the
  driver; save Ok-arm `restart_watch()` re-baselines (self-save
  hash-matches ⇒ no self-reload); watch failure ⇒ `reload_disabled`.
  `reload_from_disk` opens `reload_source()` = NEWEST of
  display/sidecar/companion `.dsl` (ties ⇒ sidecar); folds overlays →
  `reconcile` → re-keys overlays via `Report.remapped_elements`/
  `remapped_relationships` (PER KIND) → `adopt` → restore overlays +
  `rebuild_scene()`; failed reparse keeps last-good + note.
  `companion_dsl` keeps an opened sidecar's `.dsl` watched.

## M3 editing core (commands.rs + shell — read before touching edits)

- `LayoutOverlay { view_key, elements (top-lefts), relationships
  (interiors), dimensions: Option<Dimensions> (Copy!) }`; readers
  `position`/`rect`/`interior` overlay-first; `as_overrides()` →
  `scene::build` 5th param. `Command` set symmetric `apply(forward)`;
  factories ALL `Option<Command>`; `History` cap 200; per-view
  overlay/history/dirty, cleared at adopt only.
- SAVE (M3-5): `apply_overlay` writes elementViews x/y +
  relationshipViews vertices + dimensions (`to_px` round+clamp i32),
  walks the 7 MODEL-view vecs by `base.key`; filtered/image ⇒ save
  BLOCKED (`current_view_filtered`). `save_target`: `.json` in place,
  `.dsl` → sidecar, `-` → None. `write_workspace` =
  `oxidizr_json::to_vec` + `write_atomic`. BYTE STABILITY test:
  `to_vec(from_slice(bytes)) == bytes` (writer sorts by id — struct
  equality is NOT the contract). Ctrl+S focus-guarded; Save iff dirty.

## Clippy/lint catalog (cumulative — `ci gui` alone catches shell docs)

- egui 0.37: `PointerButton::Primary`; `i.pointer.any_down()`;
  `Ui::toggle_value`; `rect_stroke(rect, rounding, stroke,
  StrokeKind)`; `Id::new` DEPRECATED (`response.has_focus()`);
  `double_clicked()`; `ui.add_enabled(bool, egui::Button::new(..))`;
  `Context::request_repaint()` IS the cross-thread wake; NO
  `strong_noninteractive_color`; `Shape::translate(&mut self, Vec2)`;
  `CornerRadius` = `nw/ne/sw/se: u8`; `Shape::line` = OPEN Path.
- `similar_names` DENY (`tiles`/`tiled` tripped it); `type_complexity`
  ⇒ alias or fn-`#[allow]` + comment; `redundant_clone` early-return;
  `redundant_closure` (is_some_and(T::method));
  `items_after_statements` — fn-body `use` goes FIRST;
  `assigning_clones`/`clone_on_copy` — `Dimensions` IS Copy;
  `doc_lazy_continuation` for doc lines starting `+`; E0282 on bare
  `Vec::new()` in assert_eq ⇒ bind typed local or `Vec::<T>::new()`; `cargo fmt` reflows —
  match post-fmt text; `manual_div_ceil` ⇒ `.div_ceil()`; usize→f64/
  u32 casts ⇒ `f64::from(u32::try_from(x).unwrap())`;
  `too_many_lines` — split a helper (M5-1) before allowing.
- `format_push_string` ⇒ `let _ = write!`/`writeln!` (a format ending
  in `\n` MUST be `writeln!`); `trivially_copy_pass_by_ref` — `Stroke`
  (8 B) by VALUE; `f32: TryFrom<f64>` is NOT std — own saturating
  helper; `doc_markdown` backticks (`ab_glyph`); Result fns need
  `# Errors`; `collapsible_if` via let-chains; `too_many_lines` allow
  needs a rationale comment; unused `mut` when a cfg block vanishes —
  restructure instead of `#[allow]`; `useless_vec` on a fixed-size test
  fixture ⇒ array (the M11 spread test tripped it AFTER the sweep —
  re-run `clippy --workspace --all-targets` after adding tests);
  `filter_map(|e| e.ok())` ⇒ `filter_map(Result::ok)`.
- R-2 guard: lib core NEVER references eframe/egui (literals in DOC
  COMMENTS fail); the skip-set is the two `gui`-gated shell files
  {`main.rs`, `shell.rs`} (§P3-M8); count assert = lib.rs + N modules —
  NOW 12 files (`statefile.rs` added at M11-12; the shell module is
  NOT a core module).

## M6 display auto-layout (layout.rs + shell — read before touching)

- Rule (mirrors structurizr web): a view whose RENDERED content (filtered
  → base) has NO non-zero stored coord opens laid out by the bundled
  Layered engine — the loader materializes a missing `location` as the
  `0`/`0` pair, so all-zeros = "file has no layout". Display is
  session-only, NEVER dirty; views WITH stored coords render verbatim
  (`autolayout_once` is GONE — it dirtied on open and only fired with a
  stored `automaticLayout`). Params = view `automaticLayout` else the
  reference LR defaults, separations FLOORED at `DISPLAY_FALLBACK`
  (rank 200/node 120, graphviz lower-bound §P3-M10: cramped `autoLayout`
  100/50 rides up; direction/vertices stay the file's; dialog +
  `AutomaticLayout::default` untouched) — NEVER the dialog's session
  params; engine = Layered only, never `dot`.
- Seam: `view_needs_layout` / `display_params` / `auto_display` /
  `seed_overlay` (save-side); `Overrides::display(auto, user)` merges the
  overlay OVER the engine PER ID. Shell: `rebuild_scene` runs the pass
  UNDER the overlay; `save_workspace` seeds untouched ids so the JSON
  persists what was displayed. Guard test: `tests/auto_display.rs`
  (big-bank-plc Components: pile → spread → pin → seed).
- **M7**: the sidebar list is partitioned by kind — `overview::section`
  (rank+header per `View` variant, C4 granularity first) +
  `overview::grouped` (stable `(rank, order, key)` sort → header runs);
  the shell renders separator + uppercase headings, rows via `view_row`.

## M8 `oxidizr view` (F10 — shell now a lib module; read before touching)

- The eframe shell MOVED `main.rs` → `src/shell.rs`, `#[cfg(feature =
  "gui")] pub mod shell` (lib). Public surface: `shell::run(Option<&Path>,
  ThemeArg) -> eframe::Result` + `pub enum ThemeArg`. `main.rs` is a thin
  argv shim (clap `Args` → `shell::run`); the CLI's `oxidizr view`
  (`oxidizr-cli` feature `gui`) calls the SAME `shell::run` in-process —
  entries cannot drift. R-2 guard skip-set is now the two shell files.
- CLI: `oxidizr-cli` gained `gui = ["dep:oxidizr-viewer",
  "oxidizr-viewer/gui"]` + `Command::View { file, --theme }`. `run()`
  branches on `View` BEFORE the pipeline (`to_request` → `Option`, None
  ONLY for view) — the GUI is NOT an `oxidizr-api::Request`. Without the
  cli `gui` feature, `view` is stable `--help` surface but exits 1 with
  the rebuild hint (`anyhow::bail!`, cfg-gated). `ViewTheme` mirrors
  `ThemeArg`.
- Gates: `cargo xtask ci gui` now clippies `-p oxidizr-viewer -p
  oxidizr-cli --features oxidizr-viewer/gui,oxidizr-cli/gui` (both);
  `.github/workflows/ci.yml` mirrors the gui clippy + the 3-OS check.

## Viewer orientation (M1/M2 shipped)

- Scene: item rect top-left x/y (or override), size = style else
  350×150; edge polyline = [source centre, interiors…, destination
  centre]; `hit::pick` items last-wins, edges tolerance-first;
  boundaries not pickable. `layout.rs` `stored_for`/`config_for`/
  `apply`/`encode`/`decode` + the M6 display seam above; boundary pad
  `BOUNDARY_PADDING = 20` (deviation). Core↔engine direction names
  differ — `layout::direction()` maps.
- eframe-0.37: unified `egui::Panel`; `Storage::set_string(k, String)`;
  `Window::open(&mut bool)`; `ctx.load_texture`;
  `ComboBox::from_id_salt`; click after a MOVED drag never fires;
  bless diff-0 = `cargo xtask e2e bless | grep -cE ": unchanged$"` ==
  307 + clean `git status testdata/`.
- `cargo test -p oxidizr-viewer` = 106 lib + 1 auto_display + 3 golden
  (+2 ignored perf); `cargo test -p oxidizr-layout` = 29 lib (E-3 lane
  pass +4; +3 ignored-style gated by `dot-backend` keep 32→36 total).
  Viewer usage lives in `crates/oxidizr-viewer/README.md` (M5-3).
