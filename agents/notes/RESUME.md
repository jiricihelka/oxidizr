# RESUME — session checkpoint (updated 2026-11-26, **P5R OPEN — R0…R6 DONE (R6 = `92a212f` `efa9494` + this stamp) — next wave R7 (verdicts PRE-MEASURED, see below). Then the v0.5.0 release CLOSE. Plan: `agents/plan/build/PHASE_5R.md`. Previous arc: Phase 4 complete, tag `v0.4.0` (local, `3c34a08`), fix `4e35ee2`; pushed by the user, never by us**

## Active now (2026-11-26)

- **Next action: wave R7 — the close review, ALL VERDICTS ALREADY MEASURED**
  (this session, read-only — just RECORD them + commit as
  `P5R-R7 (close review…)`):
  - **Topology verdict: KEEP, zero crate changes** — graph re-measured via
    `cargo metadata` (2026-11-26): leaves `ast`/`core`/`diag`/`layout`;
    `dsl`→ast,core,diag,import (real); `inspect`→core NORMAL + dsl DEV
    (corpus fixtures — matches the §Inventory "dev" bookkeeping, NOT a
    topology change); clean topological order, no cycles = the recorded DAG.
    SHOW = cite §Inventory + this re-measure in the R7 row.
  - **Stabilized-idiom sweep: EMPTY** — `map_or(false` 0,
    `unwrap_or_else(Default::default)` 0, the 14 `if let`+`if` grep hits are
    NON-collapsible shapes (clippy `collapsible_if` under `-D warnings`
    across all targets proves it); nursery `redundant_clone` active, 0
    findings, 0 `allow`s.
  - **NIGHTLY audit: CLEAN** — zero `#![feature]` usages; pin
    `nightly-2026-11-13` == NIGHTLY.md row; register rows current
    (lint ratchet + MSRV-1.88 floor).
  - **Churn-list review vs CHANGELOG**: `[Unreleased]` carries the 5 churn
    lines (R1×1 reason-text, R5×4 removals — core ids/consts, export trait,
    viewer accessors, dsl loader alias); R6 added ZERO (both sub-waves were
    private-module internals). Verify wording at the CHANGELOG cut.
- **Then CLOSE = v0.5.0 release ritual (docs/RELEASING.md)**: bump
  0.4.0→0.5.0 (workspace `Cargo.toml` + `cargo update -w` lock) —
  `cargo xtask dist plan` must still show the 3 package names + 5-target
  matrix; cut `[Unreleased]` → `## [0.5.0]` (+ date); gates: quartet +
  `ci gui` leg + full tier + bless NO-OP + **windows leg**
  (`cargo xtask ci windows`, Wine, dev shell, background, long) +
  **Category B** (`OXIDZR_STRUCTURIZR_BIN=/nix/store/
  5yil7chk3cjg3qmsabq1vzdxv0v4bwm0-structurizr-2026.10.26/bin/structurizr
  cargo xtask oracle verify`, TZ=UTC, background, slow); PROGRESS.md phase
  row + PHASE_5R.md close stamp + RESUME final; commit; **annotated tag
  `v0.5.0` LOCAL — NEVER push**.
- **R6 method lessons (carry to R7/close)**: allocation claims are DEAD on
  arrival if the bytes they "save" are refcounts (`Arc<str>` Ids) or
  output-owned values — grep proves nothing, criterion decides; two
  measured sub-waves beat the whole R0 baseline (write corpus 1.84→1.00).
- **Gui-leg rule** (pitfall `gui-leg-lint-drift.md`): the quartet never
  compiles `--features gui` — viewer/cli-gui commits run
  `cargo xtask ci gui fmt clippy test refs`.
- **P5R charter (user ruling, verbatim)**: “Behaviour and features must
  stay. Error messages can improve. Internal APIs can change to remove
  allocations and introduce more rusty concepts. Successful output must
  remain the same.” ⇒ success bytes + CLI + exit classes + OXZ frozen;
  API churn recorded in `CHANGELOG.md ## [Unreleased]` same commit;
  allocation removal needs criterion evidence; topology default KEEP.

## Workflow (measured, `agents/notes/process/parallelism.md`)

- NO subagents by default (measured memory thrash; hard cap 3). Parallelize
  only harness cycles via background shell jobs. Two cargo jobs queue on
  the target-dir lock — chain in one background command; don't poll
  background shells.
- Script hygiene: python to a file under the session tmp dir, never
  `python3 -c` with backticks/`\b` (pitfall `agent-shell-quoting.md`);
  regex replaces MUST use f-strings for `{name}`; the nix dev-shell banner
  prefixes redirected stdout — strip to the first `{` before JSON parsing.
- Gate-log reading: `grep -c` exits 1 on zero matches — printed `0`/
  `FAILED 0` is GREEN (verify `ci: all steps green` in the log body).
- Wave chains: quartet+gui → full tier → bless → doc → bench →
  `bench cmp 5R` in ONE background script (pattern:
  `/tmp/opencode/p5r-r6-waveclose.sh`); never touch tracked files while a
  bless-NO-OP chain runs.

## Gates quick reference

- Sub-piece: quartet green → commit (`cargo xtask ci` in `nix develop`).
- Viewer/gui commits: `cargo xtask ci gui fmt clippy test refs`.
- Waves: + `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e` + bless NO-OP
  (307×unchanged, `git status --porcelain testdata/` empty) +
  `cargo doc --workspace --no-deps` at 0 + `bench cmp 5R` (run
  `cargo xtask bench` in the same session first).
- Category B env: `$OXIDZR_STRUCTURIZR_BIN=/nix/store/
  5yil7chk3cjg3qmsabq1vzdxv0v4bwm0-structurizr-2026.10.26/bin/structurizr`,
  legs `TZ=UTC`, background only.
- Windows leg: `cargo xtask ci windows` (dev shell, Wine).
- Pitfalls to honor: `gui-leg-lint-drift.md` (R3),
  `msrv-bump-clippy-wave.md` (R0), `windows-path-separator-assertions.md`,
  `bench-machine-drift-ab.md`, `cargo-dist-installer-panic.md`.

## Done earlier (committed)

- **P5R-R6** (`92a212f` `efa9494` + wave stamp): allocation removal,
  bench-guided — R6a json writer emit-loop trims (`m7_json_write_corpus`
  2.0→1.09ms, 1.84→1.00; `e2e_json2json_all` 6.7→5.8ms), R6b dsl in-place
  `${}` substitution (`m7_dsl_parse_test.dsl` 529→502µs, `e2e_dsl2json_all`
  10.0→9.1ms, zero churn); inventory clusters measured EMPTY (export
  to_string = output-owned, Ctx/Id = Arc refcounts, no clone ladders);
  wave gates green incl. bless NO-OP + doc-zero + `bench cmp 5R` all
  ≤1.01 (most legs faster than baseline).
- **P5R-R5** (`b586953`…`1df9cd2`, 8 commits + wave stamp): visibility
  minimization — 189 net demotes pub→pub(crate) (viewer 159→58, export
  80→39, dsl 186→169, core 131→125, api 52→51); real removals core
  `ElementId`/`RelationshipId` + `DEFAULT_WORKSPACE_*`, export
  `WorkspaceExporter`, viewer `Overview::bounds`/`map`, dsl
  `JsonWorkspaceLoader` crate-only (+ 5 dead tok consts); module-privacy
  correction (private-submodule demotes are no-ops); test-only members
  `#[cfg(test)] pub(crate)`; viewer one justified `cfg_attr` allow; R5h
  48 doc links de-linked; wave gates green.
- **P5R-R4** (`c510fc3`, docs-only): typed-ids ZERO conversions — 48
  grep-hits = 35 test fixtures + 13 PROD text/serde/composite seams;
  layout already `NodeId`/`EdgeId`, export already `&Id`-typed.
- **P5R-R3** (`0f3fae0`…`1788247`, 15 commits + wave stamp): giant-file
  decomposition B — `plantuml/engine.rs` 2186→813, `plantuml/structurizr.rs`
  1967→1221, `layered.rs` 1736→651, `scene.rs` 2068→1533, `shell.rs`
  2980→803; all VERBATIM, roots stable; lib-core guard recursive.
- **P5R-R2** (`68e8ef9`…`21118a7`, 21 commits + wave stamp): giant-file
  decomposition A — `builder.rs`→7 modules, `viewops.rs`→5, dsl
  `engine/mod.rs` 2332→1618, `stmt_views.rs`→7.
- **P5R-R0a** (`75ce39d`): MSRV floor 1.88 + let-chain/as_chunks adoption.
- **P5R-R0b** (`eb09b52`): PHASE_5R.md charter+waves, ERROR-MESSAGES.md
  registry, PLAN/index rows, RELEASING churn convention, bench baseline
  `5R`.
- **P5R-R1** (`0127166` `0851ff0`): error-taxonomy — 4 provenance leaks
  reworded + class gated; `PathFilter::compile` → `PatternError`.
- Phase 4 complete: tag `v0.4.0` (local); M5f windows fix (`4e35ee2`).
  Bench baselines `1T`…`4T` committed.
