# PHASE_5R.md — detailed plan: large-scale idiomatic-Rust refactor, closing with the v0.5.0 release

**Status: OPEN 2026-11-25** (wave table below carries per-wave evidence rows).
Position: after Phase 4 / v0.4.0 (the first phase run on a *released* tree —
the byte contract is now defended by a published baseline, not just goldens).
Unlike P2R/P4R this is a full phase (R0…R7 waves), and it **closes with a
release**: the E-5-style ritual + local tag `v0.5.0` (user decision 2026-11-25).

Goal: make the codebase substantially more idiomatic Rust at structural
scale — module decomposition, error taxonomy, typed ids, minimal visibility,
allocation removal — while the product stays bit-identical where it must.

## P5R-D0 Charter and freeze scope (ratified 2026-11-25, user Q&A)

User ruling (verbatim): *“Behaviour and features must stay. Error messages
can improve. Internal APIs can change to remove allocations and introduce
more rusty concepts. Successful output must remain the same.”*

**Frozen (the product contract)**:
- SUCCESSFUL output bytes: workspace JSON, diagram/theme/export bytes — the
  full `testdata/` corpus + blessed e2e sidecars; `cargo xtask e2e bless`
  stays a NO-OP at every wave (the tripwire; a diff = stop, fix the cause).
- Behavior + features: CLI argv grammar, exit classes, WHICH diagnostic
  fires WHERE, OXZ codes + severities, DSL semantics (Java-era quirks
  included — mistake-fixing is NOT this phase), `testdata/` never hand-edited.
- Reference-ported message TEXT stays verbatim (compat promise): everything
  marked VERBATIM in-tree (~105 sites), the inspect message set, ported
  one-liners like “Two or more workspaces must be provided”.
- Release surface: the three dist packages (`oxidizr-cli`, `oxidizr-lsp`,
  `oxidizr-viewer`) keep their names, the 5-target matrix and the smoke
  patterns — topology work (P5R-D2/R6) may move code but not these names.

**Loosened for P5R (with guards)**:
- **Error-message improvement**: OXIDIZR-COINED diagnostic text may be
  reworded for clarity — the same commit updates its test pins and its
  `docs/diagnostics.md` row; the OXZ code, severity, exit class and the
  condition that fires it NEVER change. Guard: the provenance registry
  P5R-V1 — a message is improved only if the registry says OXIDIZR-COINED;
  unclear provenance = frozen.
- **Rust API churn**: renames/moves/splits/visibility shrinks allowed; every
  breaking-ish line lands in `CHANGELOG.md` → `## [Unreleased]` in the same
  commit (the running-list practice is RETIRED — `docs/RELEASING.md` updated
  at R0; the [Unreleased] section is cut into `## [0.5.0]` at the close).
- **Allocation removal / perf idioms**: in scope; every “this is faster”
  claim carries criterion evidence (`cargo xtask bench cmp 5R` before/after
  in the commit message; swings > ~10 % in either direction reviewed).
- **Crate topology**: in scope, default answer KEEP — changes need a measured
  rationale recorded in this file before code moves.

**Out of scope**: dependency additions/changes (std-only policy; existing
thiserror/anyhow/clap set stays), behavior “fixes” of reference mistakes,
DEVIATIONS Locked list, anything needing ADDITIONAL_FEATURES sign-off,
regexlite/pathfilter unification (P4R-D5 KEEP SPLIT stands — user did not
reopen it).

## P5R-D1 Safety net (what must stay green, when)

- Every commit: quartet (`cargo xtask ci`).
- Every wave: full tier (`OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e`) +
  bless no-op + `cargo doc --workspace --no-deps` at ZERO warnings.
- Hot-path waves (dsl/json/export/layout/api + R6 always): `bench cmp 5R`.
- Windows leg (`cargo xtask ci windows`, Wine): at the phase close AND on any
  wave whose diff touches path/filesystem assertions — the M5f lesson
  (`agents/notes/pitfalls/windows-path-separator-assertions.md`).
- Phase close: quartet + full tier + bless no-op + windows leg + Category B
  (MUST run, not skippable: the error surface changes → `oracle verify` +
  `export-verify` legs + landscape, `TZ=UTC`) + `dist plan` still clean.

## P5R-D2 Catalog — R0 seeds measured against the tree (2026-11-25)

- **Giant files** (LOC / top-level fns / inline tests):
  `oxidizr-core/src/builder.rs` 3033/106 — the parser-builder arm soup;
  `oxidizr-viewer/src/shell.rs` 2980/23 — eframe event/paint surface;
  `oxidizr-dsl/src/engine/stmt_views.rs` 2900/55(+34 tests);
  `oxidizr-dsl/src/engine/mod.rs` 2332/14+12 impls;
  `oxidizr-core/src/viewops.rs` 2225/27;
  `oxidizr-export/src/plantuml/engine.rs` 2188/20;
  `oxidizr-viewer/src/scene.rs` 2068/22;
  `oxidizr-export/src/plantuml/structurizr.rs` 1967/12.
  Decomposition = module splits behind stable crate roots (moves separate
  from rewrites; behavior edits NEVER ride a move commit).
- **Errors**: 12 `pub enum *Error` (import/layout/viewer×4/cli/export×2/json/
  ast/core + theme); the OXZ catalog lives single-file
  `oxidizr-diag/src/lib.rs`. Redesign target: uniform thiserror shape
  (`#[source]`, no hand-rolled Display unless byte-pinned), `?`-chains
  end-to-end, no `String`-typed error legs inside crates.
- **Typed ids**: 48 `id: &str|String` grep-hits, concentrated in the export
   engines (plantuml/structurizr/mermaid/wsd = 26) and layout (6) —
   R4 AUDIT CORRECTION (2026-11-25): the line-based inventory counted test
   helpers; by-module recount = 35 sites are `mod tests`/`tests/*.rs` fixture
   literals (the text→`Id::new` edge itself), 13 are production and each is a
   text/serde/composite seam (detail in the R4 wave row). Newtype candidates
   where they clarify call sites; serde/JSON shapes untouched (newtypes are
   call-site plumbing, `&str` at serde edges).
- **Visibility**: top-level `pub` density — dsl 186, viewer 159, core 131,
  export 80, api 52 — shrink audit vs the measured consumer graph below.
- **Topology (measured DAG)**: leaves `ast`,`diag`,`layout`(dep-free),
  `core`; `json`→core; `import`→core; `dsl`→ast/core/diag/**import**
  (real, not dev — the `!docs` engine hooks), dev-json; `inspect`→core+dsl;
  `export`→core+json; `api`→all feature crates; `cli`→api+viewer(optional
  gui); `lsp`→api; `viewer`→api/core/diag/json/layout; `e2e`/`xtask` dev-ish.
  R6 verdict target: the graph is a clean DAG with honest boundaries;
  expected outcome is FEW or ZERO crate changes — record honestly.
- **Nightly probe (R0, measured)**: `if let … && …` let-chains compile on
  the pinned `nightly-2026-11-13` with **no `#![feature]`** — stabilized
  (≥ 1.88). Decision recorded: the adopted idiom set is *stabilized* syntax;
  honest metadata follows — `rust-version` 1.85 → **1.88** (bookkeeping of
  the chosen idiom set, not a nightly feature), NIGHTLY.md + claims updated
  same commit. New NIGHTLY.md rows only if a genuinely-unstable idiom is
  ever wanted (none planned).
- **Bench baseline**: `5R` saved at R0 (`cargo xtask bench` +
  `bench save 5R`, committed under `bench-baseline/5R/`); comparison anchor
  for the phase (same-session A/B still the arbiter for close calls —
  `agents/notes/pitfalls/bench-machine-drift-ab.md`).

## P5R-D3 Waves (each: quartet/commit; wave end: full tier + bless no-op)

| Wave | Scope | Commit |
|---|---|---|
| **R0** | Inventory + baseline `5R` + message-provenance registry (`agents/plan/ERROR-MESSAGES.md`, family-granularity, V1) + `rust-version` 1.88 bookkeeping + NIGHTLY row + `CHANGELOG.md` `## [Unreleased]` section + `docs/RELEASING.md` churn-convention update — **DONE 2026-11-25**: R0a = MSRV floor + the clippy lint wave it fires (20 files, let-chain/`as_chunks`, pitfall note `msrv-bump-clippy-wave.md`); R0b = charter/registry/baseline (92 files); wave gates: quartet green, full tier ok, bless 307×unchanged, doc-zero, refs green | `75ce39d` `eb09b52` |
| **R1** | Error taxonomy — **DONE 2026-11-25**: defect first (R1a): FOUR shipped-text provenance leaks reworded (OXZ-5007 message, oracle text, two doc comments — all grepped UNPINNED) and the class gated: hygiene patterns `agents/`/`phase_` + fixtures; rework (R1b): `PathFilter::compile` String→`PatternError` (transparent Display, byte-stable reason; CHANGELOG Rust-API line). KEEP rows with measured reasons: CoreError/JsonError/ImportError Display = FROZEN/REFERENCE verbatim; core/layout/ast hand-rolled Display = zero-dep promises (already `impl Error`, uniform by convention); `OracleError` = the exit-family CLASSIFICATION is the contract, thiserror would need a new dep; idents ×3 = official-message-at-seam convention; query ×2 = verbatim Issue body, no typed consumer; `json_loader` = dsl↔api seam (dsl cannot name json errors); no remaining IMPROVABLE text justifies churn. Wave end: quartet green, full tier 8 suites 0 FAILED, bless 307×unchanged, `bench cmp 5R` all ≤1.05 (noise), doc-zero held | `0127166` `0851ff0` |
| **R2** | Giant-file decomposition A — **DONE 2026-11-25**: 21 move-only commits (R2a…R2u), every cluster VERBATIM (zero behavior edits inside moves — behavior edits NEVER ride a move): `builder.rs` 3033 → `builder/` root 1069 (views/deployment/relationships/elements/setters/lookups), `viewops.rs` 2225 → root 1043 (gates/content/defaults/steps), dsl `engine/mod.rs` 2332 → 1618 (defs/archetypes/subst/holders/filectx), `stmt_views.rs` 2900 → `stmt_views/` root 1464 (content/defs/steps/config/dynamic/theme); roots stable (`oxidizr_core::builder::WorkspaceBuilder`, `crate::builder::`, `engine::` paths unchanged, `git mv` rename-detectable); parent keeps the shared free helpers (children resolve through `super::`; a directory-of-a-directory reaches engine items through `super::super::`); deliberate `private → pub(super)` bumps ONLY where the staying dispatcher or a sibling module calls in, recorded per commit; `cargo fix` prunes the dead parent imports. Wave end: quartet green every commit; ci green, full tier 8 suites 0 FAILED, bless 307×unchanged NO-OP, doc-zero, `bench cmp 5R` within noise (dsl hot ≤1.02; worst m7_json_write 1.09 on a leg this wave never touched) | `68e8ef9`…`21118a7` |
| **R3** | Giant-file decomposition B — **DONE 2026-11-25**: 15 move-only commits (R3a…R3n + R3j-lint), every cluster VERBATIM (zero behavior edits inside moves): `plantuml/engine.rs` 2186 → `engine/` 813 (dialect/write/query, `pub use dialect::{Dialect, Frame}` keeps paths stable), `plantuml/structurizr.rs` 1967 → `structurizr/` 1221 (style/legs, `pub(crate) use legs::{…}` keeps the c4/mermaid/wsd imports), `layered.rs` 1736 → `layered/` 651 (skeleton/routing, skeleton reaches routing through `super::routing::{…}`), `scene.rs` 2068 → `scene/` 1533 (resolve/label), `shell.rs` 2980 → `shell/` 803 (canvas/browser/actions/widgets/panels — the App wiring + Viewer state + path helpers stay); roots stable, `git mv` rename-detectable; same helper/`pub(super)`-bump rules as R2. Viewer guard: the lib-core shell-boundary test became a RECURSIVE walk (directory modules guarded file by file) and now SKIPS the `shell` dir (the split keeps the one-place rule), count pinned 14. Gui-leg drift found: 10 `collapsible_if` errors on the `ci gui` leg proven PRE-EXISTING at HEAD (stash check) — fixed as the SEPARATE zero-behavior commit R3j-lint, rule recorded in pitfall note `gui-leg-lint-drift.md`; every shell commit since runs the gui leg. Wave end: quartet green every commit; ci gui green, full tier 8 suites 0 FAILED, bless 307×unchanged NO-OP, doc-zero, `bench cmp 5R` all ≤1.07 (export/layout hot legs flat-to-faster) | `0f3fae0`…`1788247` |
| **R4** | Typed ids/newtypes in the export engines + layout (the 48-site concentration); serde edges stay `&str`; bench cmp mandatory — **DONE 2026-11-25, verdict: zero conversions (measured)**. By-module recount: 48 grep-hits = 35 TEST fixture helpers (`mod tests` + `tests/*.rs` — the literal→`Id::new` edge; `&str` is the idiomatic fixture input, wrapping adds noise not clarity) + 13 PROD, all KEEP with reasons: layout PROD **already has newtypes** `NodeId`/`EdgeId` (`graph.rs:11,35` — the dep-free leaf's design: layout cannot name core `Id`); export PROD **already `&Id`-typed** (30 `&Id`/`Id` params export+layout prod); remaining prod `&str`/`String` are text seams — `revealed_by_step` (id from a frame STRING, DSL text edge), `view_name`/`id_of` → `String` = OUTPUT-text domain (reference-ported bytes), `docs.rs` `Decision.id`/`Link.id` = Structurizr JSON free-form decision ids (byte contract; sibling `element_id` is `Option<Id>` — core distinguishes the domains the reference does), `calculate_url` = reference fidelity port, `api Row.id` = report row formatted to output bytes (mixed model-`Id`/view-key sources), `nav_matches` id = SEARCH needle not identity, scene `Boundary.id`+`GroupSpec`/`ScopeSpec.id`+`boundary_from` = COMPOSITE ids (`group:<scope>/<name>`, `deployment:<id>`) — `Item.id`/`Node.id`/`DnSpec.id` already `Id`, `inspect` `element_holders`/`element_severity` = the inspector's own raw-id report maps, dsl `rel.rs` find/`token_id` = parser token text pre-registration (R1 idents-KEEP seam, `Id` is the return type); dsl/rel, import, inspect, api, viewer, core, docs verified too. bench cmp 5R VACUOUS (zero code change — wave diff docs-only); gate = refs green | (docs-only — this stamp) |
| **R5** | Visibility minimization — **DONE 2026-11-25**: 8 commits (`b586953`…`1df9cd2`): audit = inverted token index over every workspace `.rs` (bin targets + integration tests counted EXTERNAL), per-NAME `pub use` rebuilds, compiler/clippy as final judge. Net **189 `pub`→`pub(crate)` demotes**; density before→after: viewer 159→58, export 80→39, dsl 186→169, core 131→125, api 52→51. REAL REMOVALS (CHANGELOG churn lines): core `ids::ElementId`/`RelationshipId` + `DEFAULT_WORKSPACE_NAME`/`DESCRIPTION`; export `diagram::WorkspaceExporter` trait (never implemented); viewer `Overview::bounds`/`map` (zero callers); dsl `engine::JsonWorkspaceLoader` crate-only. KEEP-family evidence: the signature-leak blind spot per batch (types reached through pub signatures/inference — ast `LosslessnessError`, `StdinReader`, `FileId`/`CodeRange`, `IdChange`, layout `NodePlacement`/`EdgePlacement`/`Anchor`, `ParseThresholdError`, core `ValidationError`/`ElementMut`, cli clap-derive enum family (rides pub `Command` fields), e2e manifest report family, export `ExportedFile`/`IndentingWriter`/`IndentType`, viewer scene/hit/icons plumbing 10 types, dsl `CursorScope`/`Logical`/`Segment`/`LexedLine`); **module-privacy correction (R5g): demotes inside PRIVATE submodules are no-ops** — 93 tok.rs consts + idents/ctx/uuid/tokenize/tokens reverted, audit DEMOTE rows there reclassified KEEP (the `pub` keyword was already capped by `mod` privacy). Test-only members on demoted types → `#[cfg(test)] pub(crate)` + justification (lib-target dead_code is not silenced by unit tests). Viewer headless core: one crate-level `#![cfg_attr(not(feature = "gui"), allow(dead_code))]` (the gui shell is its only non-test consumer). R5h: 48 intra-doc links de-linked (rustdoc private-item warnings) restoring doc-zero. Wave end: quartet green every commit (gui leg on viewer/cli/dsl), full tier 8 suites 0 FAILED, bless 307×unchanged NO-OP, doc-zero, `bench cmp 5R` all ≤1.06 (majority faster than the `5R` baseline — visibility shrink is codegen-neutral to-positive) | `b586953`…`1df9cd2` |
| **R6** | Allocation-removal pass, bench-guided — **DONE 2026-11-26**: 2 commits (`92a212f` R6a, `efa9494` R6b), every claim critcmp-evidenced in the same session (baseline `5R` untouched — no `bench save` mid-phase). **R6a** json writer: `Json::Num` renders via `write!` into the byte buffer, `pad` resizes straight to spaces, `escape_into` walks bytes (non-ASCII copies verbatim = same bytes as the char-wise re-encode) — `m7_json_write_corpus` 2.0→1.09 ms (1.84→1.00), `e2e_json2json_all` 6.7→5.8 ms. **R6b** dsl engine: `${}` substitution runs IN PLACE on the tokenizer's own Strings (`substitute_in`; tokens without `${` never copied; `substitute_strings` kept as the cold wrapper) — `m7_dsl_parse_test.dsl` 529→502 µs, `e2e_dsl2json_all` 10.0→9.1 ms, no public churn (private `engine` submodules, no CHANGELOG line). **Inventory clusters measured EMPTY** (negative evidence, no change attempted — charter: measured or absent): export-path `to_string()`s are output-OWNED (`Diagram`/`ModelItem` own their strings), `Ctx`/`Id` clones are `Arc<str>` refcount bumps (no string allocation), no collect-then-clone ladders exist in hot paths. Wave end: quartet+gui green, full tier 8 suites 0 FAILED, bless 307×unchanged NO-OP, doc-zero, `bench cmp 5R` **every leg ≤ 1.01** — json writer, dsl parse, json read, validate, reconcile and layered legs all FASTER than the R0 baseline | `92a212f` `efa9494` |
| **R7** | Topology verdict + close review — **DONE 2026-11-26** (measurements taken this session, recorded here; zero code changes by verdict). **Topology: KEEP, zero crate changes** — graph re-measured via `cargo metadata` (normal deps): leaves `ast`/`core`/`diag`/`layout`; `json`/`import`→core; `dsl`→ast,core,diag,import (real); `inspect`→core (its dsl dep is DEV — corpus fixtures, matching the §Inventory dev bookkeeping, NOT a topology change); `export`→core,json; `api`→all feature crates; `viewer`→api,core,diag,json,layout; `cli`→api..viewer; `lsp`→api..json; clean topological order, **no cycles = the recorded DAG holds** (§Inventory SHOW). **Stabilized-idiom sweep: EMPTY** — `map_or(false` 0 hits, `unwrap_or_else(Default::default)` 0, the 14 remaining `if let`+`if` line-adjacencies are NON-collapsible shapes (`clippy::collapsible_if` runs under `-D warnings` on every gate incl. tests), nursery `redundant_clone` active with 0 findings and 0 suppressions. **NIGHTLY audit: CLEAN** — zero `#![feature]` usages in code; `rust-toolchain.toml` pin `nightly-2026-11-13` == the NIGHTLY.md register row; both rows current (lint ratchet + MSRV-1.88 floor). **Churn-list review**: `CHANGELOG.md [Unreleased]` carries exactly the 5 real-churn lines (R1 error-reason text + R5's four removals); R2–R4/R6 additions were internal-only by the RELEASING.md convention — nothing to add, wording re-checked at the v0.5.0 cut | this commit |
| **Close** | v0.5.0 release ritual per `docs/RELEASING.md`: bump `0.4.0`→`0.5.0` (+ lock), cut `[Unreleased]` → `## [0.5.0] - date`, docs pass, gates: quartet + full tier + bless no-op + **windows leg** + **Category B (mandatory)** + `dist plan`; PROGRESS board/log + PHASE_5R close stamps + RESUME; LOCAL annotated tag `v0.5.0`. **Never push** | |

## P5R-D4 Verification list

- **P5R-V1** — message-provenance registry `agents/plan/ERROR-MESSAGES.md`
  exists (R0) and every message a wave rewords maps to an
  `OXIDIZR-COINED` row; REFERENCE-PORTED/`VERBATIM`/catalog-golden rows
  untouched. — OPEN
- **P5R-V2** — bless NO-OP at every wave end (307×unchanged; success bytes
  untouched). — OPEN
- **P5R-V3** — `5R` baseline committed at R0; every hot wave carries
  `bench cmp 5R`; swings > ~10 % reviewed in the wave row. — OPEN
- **P5R-V4** — dist release surface unchanged through the phase:
  `dist plan` clean at R0/R6/close; three package names + 5 targets intact. — OPEN
- **P5R-V5** — windows leg green at close (+ any fs-touching wave). — OPEN
- **P5R-V6** — `cargo doc --workspace --no-deps` stays 0 warnings; refs gate
  green at every commit (P4R-V2 carry-over). — OPEN
- **P5R-V7** — `rust-version`/NIGHTLY.md/README claims truthful after the
  idiom adoption (no false stable-compat claim). — OPEN

## Risks

- **Error-text drift vs the reference** — the biggest loosening: guarded by
  V1 registry + the VERBATIM in-tree markers + Category B at close; a wave
  that changes a ported message is red by definition.
- **Module-split sprawl** (git churn masking real changes): moves in their
  own commits (`git mv`), zero behavior edits inside moves, roots re-export
  so consumers/tests do not churn.
- **Perf claims without evidence**: V3 makes the bench output part of the
  commit message; regressions > 10 % block the wave until reviewed.
- **Topology ambition without need**: default KEEP; R7 must SHOW the measured
  reason or record zero changes.
- **API churn sprawl** (the 2R lesson): one churn item per commit, every
  line into CHANGELOG [Unreleased] same commit; nothing “blesses” a product
  diff — the freeze table overrides convenience.
- **Release at close**: v0.5.0 is tagged LOCALLY; push/publish stay the
  user’s explicit actions (`docs/RELEASING.md`).
