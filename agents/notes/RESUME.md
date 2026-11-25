# RESUME — session checkpoint (updated 2026-11-25, **P5R OPEN — R0 + R1 + R2 + R3 + R4 + R5 DONE (R5 = 8 commits `b586953`…`1df9cd2` + this stamp) — next wave R6 allocation removal. Plan: `agents/plan/build/PHASE_5R.md`. Waves R6…R7 + release close (v0.5.0, LOCAL tag). Previous arc: Phase 4 complete, tag `v0.4.0` (local, `3c34a08`), fix `4e35ee2`; pushed by the user, never by us**

## Active now (2026-11-25)

- **Next action: wave R6 — allocation-removal pass, bench-guided** (PHASE_5R.md
  D3): targets named by the inventory — clones in the export path
  (`crates/oxidizr-export/src/plantuml/*`, `mermaid.rs`, `wsd.rs`),
  collect-then-clone ladders, String churn in `crates/oxidizr-dsl/src/engine/*`.
  Rules: EVERY claim = critcmp evidence (`cargo xtask bench` then
  `cargo xtask bench cmp 5R` — never `bench save` mid-phase); a "removal"
  that does not measure better is REVERTED (charter: measured or absent).
  Success bytes stay identical (bless 307× NO-OP at wave end proves it).
  Rust-API churn lines into `CHANGELOG.md ## [Unreleased]` if internal
  signatures change. Start by profiling candidates: grep `\.clone()` +
  `to_string()` + `collect::<Vec` in export/dsl hot paths, keep the
  serde/JSON byte edges untouched (borrowing `&str` out of held `Id`/maps
  is the main lever).
- **R5 method + leftovers to carry**: the token audit over-counted —
  module privacy caps `pub` inside private submodules (demotes there are
  NO-OPs, R5g lesson); signature-leaked types must stay pub (compiler
  `private_interfaces`/E0365/E0446 = the judge). Leftover KEEP rows worth
  remembering: viewer headless core has ONE crate-level
  `#![cfg_attr(not(feature = "gui"), allow(dead_code))]` (shell is the
  only non-test consumer); viewer test-only members are
  `#[cfg(test)] pub(crate)` + comment; cli clap-derive enum family is pub
  by design (rides `Command` fields).
- **Gui-leg rule** (pitfall `gui-leg-lint-drift.md`): the quartet never compiles
  `--features gui` code — viewer/cli-gui commits run
  `cargo xtask ci gui fmt clippy test refs` (gui leg compiles shell only under
  the feature); pre-existing gui drift gets its OWN zero-behavior commit.
- **P5R charter (user ruling, verbatim)**: “Behaviour and features must
  stay. Error messages can improve. Internal APIs can change to remove
  allocations and introduce more rusty concepts. Successful output must
  remain the same.” ⇒ success bytes + CLI + exit classes + OXZ frozen;
  OXIDIZR-COINED error TEXT improvable behind the registry; API churn
  recorded in `CHANGELOG.md ## [Unreleased]` same commit; allocation
  removal needs criterion evidence; topology default KEEP.
- **After R6**: R7 = topology verdict (measured DAG already in PHASE_5R.md
  §Inventory — default KEEP, SHOW the measurement) + stabilized-idiom
  leftover sweep + NIGHTLY audit + churn-list review vs CHANGELOG; then
  the v0.5.0 release close (bump 0.4.0→0.5.0 + lock, cut `[Unreleased]` →
  `## [0.5.0]`, quartet + full tier + bless NO-OP + gui leg + WINDOWS leg
  + **Category B** + `dist plan`, PROGRESS/PHASE/RESUME stamps, LOCAL
  annotated tag `v0.5.0`). NEVER push.

## Workflow (measured, `agents/notes/process/parallelism.md`)

- NO subagents by default (measured memory thrash; hard cap 3). Parallelize
  only harness cycles via background shell jobs. Two cargo jobs queue on
  the target-dir lock — chain in one background command; don't poll
  background shells.
- Script hygiene: python to a file under the session tmp dir, never
  `python3 -c` with backticks/`\b` (pitfall `agent-shell-quoting.md`);
  regex replaces MUST use f-strings for `{name}` (R5f wrote a literal
  `{name}` into three signatures with a raw-string — repaired by
  `/tmp/opencode/p5r-r5-repair.py` the same session); long-line doc edits
  via anchored `sed -i 'Nr file'`.
- Gate-log reading: `grep -c` exits 1 on zero matches — the blessed
  `R0-GATES EXIT: 1` after a printed `0` doc-warnings count is GREEN
  (verify `ci: all steps green` + `0 FAILED` in the log body instead).
- Wave chains: chain quartet+gui → full tier → bless → doc → bench →
  `bench cmp 5R` in ONE background script (pattern:
  `/tmp/opencode/p5r-r5-waveclose.sh`); never touch tracked files while a
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

- **P5R-R5** (`b586953`…`1df9cd2`, 8 commits + wave stamp): visibility
  minimization — 189 net demotes pub→pub(crate) (viewer 159→58, export
  80→39, dsl 186→169, core 131→125, api 52→51); real removals core
  `ElementId`/`RelationshipId` + `DEFAULT_WORKSPACE_*`, export
  `WorkspaceExporter`, viewer `Overview::bounds`/`map`, dsl
  `JsonWorkspaceLoader` crate-only (+ 5 dead tok consts); module-privacy
  correction (private-submodule demotes are no-ops — 93 tok + 10
  engine/legs reverted); test-only members `#[cfg(test)] pub(crate)`;
  viewer one justified `cfg_attr` allow; R5h 48 doc links de-linked;
  wave gates: quartet every commit, gui leg on viewer/cli/dsl, full tier
  8/8, bless NO-OP, doc-zero, bench cmp 5R all ≤1.06 (most faster).
- **P5R-R4** (`c510fc3`, docs-only): typed-ids ZERO conversions — 48
  grep-hits = 35 test fixtures + 13 PROD text/serde/composite seams;
  layout already `NodeId`/`EdgeId`, export already `&Id`-typed.
- **P5R-R3** (`0f3fae0`…`1788247`, 15 commits + wave stamp): giant-file
  decomposition B — `plantuml/engine.rs` 2186→813 (+dialect/write/query),
  `plantuml/structurizr.rs` 1967→1221 (+style/legs), `layered.rs`
  1736→651 (+skeleton/routing), `scene.rs` 2068→1533 (+resolve/label),
  `shell.rs` 2980→803 (+canvas/browser/actions/widgets/panels); all
  VERBATIM, roots stable; lib-core guard recursive (14 files, shell
  skipped); gui drift fixed as separate zero-behavior commit.
- **P5R-R2** (`68e8ef9`…`21118a7`, 21 commits + wave stamp): giant-file
  decomposition A — `builder.rs`→7 modules (root 1069), `viewops.rs`→5
  (1043), dsl `engine/mod.rs` 2332→1618, `stmt_views.rs`→7 (root 1464).
- **P5R-R0a** (`75ce39d`): MSRV floor 1.88 + let-chain/as_chunks adoption,
  NIGHTLY row, CHANGELOG [Unreleased] seeded, pitfall note.
- **P5R-R0b** (`eb09b52`): PHASE_5R.md charter+waves, ERROR-MESSAGES.md
  provenance registry, PLAN/index rows, RELEASING churn convention,
  bench baseline `5R`.
- **P5R-R1** (`0127166` `0851ff0`): error-taxonomy — 4 provenance leaks
  reworded + class gated; `PathFilter::compile` → `PatternError`;
  KEEP rows for FROZEN/REFERENCE Displays.
- Phase 4 complete: tag `v0.4.0` (local); M5f windows fix (`4e35ee2`).
  Bench baselines `1T`…`4T` committed.
