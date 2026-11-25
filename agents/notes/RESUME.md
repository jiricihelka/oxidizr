# RESUME — session checkpoint (updated 2026-11-25, **P5R OPEN — R0 + R1 + R2 DONE (R2 = 21 move-only commits `68e8ef9`…`21118a7` + this stamp) — next wave R3 decomposition of export+layout+viewer. Plan: `agents/plan/build/PHASE_5R.md`. Waves R3…R7 + release close (v0.5.0, LOCAL tag). Previous arc: Phase 4 complete, tag `v0.4.0` (local, `3c34a08`), fix `4e35ee2`; pushed by the user, never by us**

## Active now (2026-11-25)

- **Next action: wave R3 — giant-file decomposition B** (PHASE_5R.md D3):
  `plantuml/engine.rs` 2186 (`Dialect` trait 51–228, `Engine` struct+ctor
  231–~310, write+sequence legs, query/visibility legs, free text helpers
  1575–1652 — some `pub(crate)`-used by mermaid/c4/wsd, keep those import
  paths stable via mod re-exports — tests tail); `plantuml/structurizr.rs`
  1967 (constants + `StructurizrDialect` + 716-line `impl Dialect` +
  `PlantUMLStyle` impl + naming/id legs `view_name`/`id_of`/… consumed by
  c4/mermaid/wsd); `layout/layered.rs` 1736 (Skeleton pipeline 83–642 +
  routing legs 651–1165 — `place()` calls route/port_spread/spread_channels
  across the seam → `pub(super)`; only `Layered` is exported); viewer
  `shell.rs` 2980 (Viewer structs + 3 impl blocks + canvas/panels/actions
  legs + ~350 lines of free widget fns at the tail; NO tests in shell.rs)
  and `scene.rs` 2068 (public types + `build()` + resolve/paint/title legs
  + tests — public `scene::build` path stable). Same rules as R2:
  `git mv`/verbatim moves, ZERO behavior edits, bench cmp 5R at wave end
  (export/layout hot).
- **R2 method (proven, 21 commits)**: verbatim line-range extraction;
  behavior edits NEVER ride moves; parent private helpers STAY in the
  parent (children see them through `super::`, nested grandchildren
  re-qualify `super::x` → `super::super::x` as allowed text edits,
  recorded per commit); child→parent-remaining calls get `pub(super)`
  bumps (deliberate internal churn, in the commit message); parent wires
  each child with `mod x;` + `use x::{arms};`; `cargo fix --allow-dirty`
  prunes dead parent imports; `impl`-wrapper headers in /tmp/opencode.
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
- **Wave order after R2**: R3 export+layout+viewer (shell.rs, scene.rs,
  plantuml, layered); R4 typed ids (export-heavy, 48 sites, bench
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

## Gates quick reference

- Sub-piece: quartet green → commit (`cargo xtask ci` in `nix develop`).
- Waves: + `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e` + bless NO-OP
  (307×unchanged, `git status --porcelain testdata/` empty) +
  `cargo doc --workspace --no-deps` at 0.
- Category B env: `$OXIDZR_STRUCTURIZR_BIN=/nix/store/
  5yil7chk3cjg3qmsabq1vzdxv0v4bwm0-structurizr-2026.10.26/bin/structurizr`,
  legs `TZ=UTC`, background only.
- Windows leg: `cargo xtask ci windows` (dev shell, Wine).
- Pitfalls to honor: `msrv-bump-clippy-wave.md` (R0 lesson),
  `windows-path-separator-assertions.md`, `bench-machine-drift-ab.md`,
  `cargo-dist-installer-panic.md`.

## Done earlier (committed)

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
