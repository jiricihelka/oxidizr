# RESUME — session checkpoint (updated 2026-11-25, **P5R OPEN — R0 + R1 + R2 + R3 DONE (R3 = 15 move-only commits `0f3fae0`…`1788247` incl. R3j-lint + this stamp) — next wave R4 typed ids. Plan: `agents/plan/build/PHASE_5R.md`. Waves R4…R7 + release close (v0.5.0, LOCAL tag). Previous arc: Phase 4 complete, tag `v0.4.0` (local, `3c34a08`), fix `4e35ee2`; pushed by the user, never by us**

## Active now (2026-11-25)

- **Next action: wave R4 — typed ids** (PHASE_5R.md D3): re-measured 2026-11-25,
  48 `id: &str|String` params outside tests — export engines 26 (`plantuml/engine/mod.rs` 7,
  `wsd.rs` 6, `plantuml/structurizr/mod.rs` 6, `mermaid.rs` 6, `engine/query.rs` 1),
  layout 6 (`layered/mod.rs` 4, `graph.rs` 2), rest scattered (viewer 7, inspect 2,
  dsl 2, core 2, import 1, api 1). Rule: newtypes ONLY where call sites clarify
  (e.g. one id-domain per engine leg — element-id vs relationship-id confusion is
  the bug class); serde/JSON edges stay `&str` (byte contract); `&str`→`&Id` where
  `oxidizr_core::Id` already flows. `bench cmp 5R` mandatory at wave end; Rust-API
  churn lines into `CHANGELOG.md ## [Unreleased]` (internal-API change — unlike
  R2/R3 which were structural only).
- **R3 method (proven, 15 commits)**: same as R2 + viewer lessons: extraction
  headers wrap methods in `impl X {`; after a move + `cargo fix`, the `mod tests`
  block often breaks (pruned parent import no longer reaches `use super::*`) —
  fix with explicit imports IN the tests block (seen: `ModelItem`, `Model`,
  `DeploymentNode`, `ElementView`); `pub(super) use` of a child-PRIVATE fn fails
  E0364 — import sibling paths instead; the viewer lib-core guard is now a
  RECURSIVE walk pinning 14 lib-core files and SKIPS the `shell` dir — bump the
  count ONLY when a lib-core (non-shell) file is added.
- **Gui-leg rule** (pitfall `gui-leg-lint-drift.md`): the quartet never compiles
  `--features gui` code — viewer/cli-gui commits run
  `cargo xtask ci gui fmt clippy test refs` (gui leg compiles shell only under
  the feature); pre-existing gui drift gets its OWN zero-behavior commit.
- **R1 verdicts to remember**: hand-rolled Displays at core/layout/ast and
  `OracleError` are KEEP (zero-dep promises / exit-family classification =
  contract); idents/query/json_loader String legs are KEEP at documented
  seams; rewording any message requires the ERROR-MESSAGES.md registry walk.
- **P5R charter (user ruling, verbatim)**: “Behaviour and features must
  stay. Error messages can improve. Internal APIs can change to remove
  allocations and introduce more rusty concepts. Successful output must
  remain the same.” ⇒ success bytes + CLI + exit classes + OXZ frozen;
  OXIDIZR-COINED error TEXT improvable behind the registry; API churn
  recorded in `CHANGELOG.md ## [Unreleased]` same commit (running list
  retired — RELEASING.md updated); allocation removal needs criterion
  evidence; topology default KEEP.
- **Wave order after R3**: R4 typed ids (export-heavy, 48 sites, bench
  mandatory); R5 visibility shrink; R6 allocation removal (bench cmp
  mandatory); R7 topology verdict + idiom sweep + NIGHTLY audit; then
  v0.5.0 release close (bump, CHANGELOG cut, windows leg, **Category B
  mandatory**, dist plan, LOCAL tag).
- **Gates**: quartet/commit; full tier + bless NO-OP (307×unchanged) +
  doc-zero per wave; `bench cmp 5R` on hot waves; Windows leg at close +
  fs-touching waves; NEVER push.

## Workflow (measured, `agents/notes/process/parallelism.md`)

- NO subagents by default (measured memory thrash; hard cap 3). Parallelize
  only harness cycles via background shell jobs. Two cargo jobs queue on
  the target-dir lock — chain in one background command; don't poll
  background shells.
- Script hygiene: python to a file under the session tmp dir, never
  `python3 -c` with backticks/`\b` (pitfall `agent-shell-quoting.md`);
  long-line doc edits via anchored `sed -i 'Nr file'`.
- Gate-log reading: `grep -c` exits 1 on zero matches — the blessed
  `R0-GATES EXIT: 1` after a printed `0` doc-warnings count is GREEN
  (verify `ci: all steps green` + `0 FAILED` in the log body instead).
- Split-script anchor hygiene: the `mod <stem>;` anchor insert SHIFTS line
  numbers of everything below it — anchor ABOVE the extraction range or
  extract first, wire the `use` list after (R3i clipped `build()`'s tail
  this way; revert + redo cost 2 minutes).

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

- **P5R-R3** (`0f3fae0`…`1788247`, 15 commits + wave stamp): giant-file
  decomposition B — `plantuml/engine.rs` 2186→813 (+dialect/write/query),
  `plantuml/structurizr.rs` 1967→1221 (+style/legs), `layered.rs`
  1736→651 (+skeleton/routing), `scene.rs` 2068→1533 (+resolve/label),
  `shell.rs` 2980→803 (+canvas/browser/actions/widgets/panels); all
  VERBATIM, roots stable; lib-core guard made recursive (14 files, shell
  skipped); gui drift fixed as separate zero-behavior commit; wave gates:
  quartet every commit + gui leg on shell commits, full tier 8/8, bless
  NO-OP, doc-zero, bench cmp 5R all ≤1.07 (hot legs flat-to-faster).
- **P5R-R2** (`68e8ef9`…`21118a7`, 21 commits + wave stamp): giant-file
  decomposition A — `builder.rs`→7 modules (root 1069), `viewops.rs`→5
  (1043), dsl `engine/mod.rs` 2332→1618 (+defs/archetypes/subst/holders/
  filectx), `stmt_views.rs`→7 modules (root 1464); all VERBATIM moves,
  roots stable; wave gates: quartet every commit, full tier 8/8, bless
  NO-OP, doc-zero, bench cmp 5R noise-level.
- **P5R-R0a** (`75ce39d`): MSRV floor 1.88 + let-chain/as_chunks adoption
  (20 code files, clippy lint wave fixed same-commit), NIGHTLY row,
  CHANGELOG [Unreleased] seeded, pitfall note.
- **P5R-R0b** (`eb09b52`): PHASE_5R.md charter+waves, ERROR-MESSAGES.md
  provenance registry, PLAN/index rows, RELEASING churn convention,
  bench baseline `5R` (92 files, saved from the pre-P5R tree — R0a is
  codegen-neutral syntax only).
- Phase 4 complete: tag `v0.4.0` (local); M5f windows fix (`4e35ee2`).
  Bench baselines `1T`…`4T` committed.
