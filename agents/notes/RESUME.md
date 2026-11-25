# RESUME — session checkpoint (updated 2026-11-25, **P5R OPEN — R0 DONE (R0a `75ce39d` MSRV floor + R0b charter/baseline) — next wave R1 error taxonomy. Plan: `agents/plan/build/PHASE_5R.md`. Waves R0…R7 + release close (v0.5.0, LOCAL tag). Previous arc: Phase 4 complete, tag `v0.4.0` (local, `3c34a08`), post-close fix `4e35ee2`; all pushed by the user, never by us**

## Active now (2026-11-25)

- **Next action: wave R1 — error taxonomy** (PHASE_5R.md D3 row carries the
  surveyed targets): (a) FIX FIRST `ExportError::NotImplemented` Display —
  ships `agents/plan/build/PHASE_2.md` provenance (text unpinned anywhere →
  safe reword) + extend `check_hygiene` (xtask refs) patterns to catch such
  leaks (`agents/` paths + `PHASE_\\d` in shipped string literals, fixture
  test); (b) uniform error shapes where deps allow (thiserror already at
  json/export×2/import/viewer×4; hand-rolled std Display at core/layout/ast
  STAYS — documented zero-dep promises; `OracleError` may gain source chains);
  (c) String-error legs: `api/generate.rs` 8 + `api/query.rs` 2 +
  `dsl/engine/idents.rs` 3 real targets (e2e dev-crate = lower bar);
  (d) CoreError/ImportError/JsonError texts FROZEN/REFERENCE — Display bytes
  byte-identical (registry `agents/plan/ERROR-MESSAGES.md`, P5R-V1).
- **P5R charter (user ruling, verbatim)**: “Behaviour and features must
  stay. Error messages can improve. Internal APIs can change to remove
  allocations and introduce more rusty concepts. Successful output must
  remain the same.” ⇒ success bytes + CLI + exit classes + OXZ frozen;
  OXIDIZR-COINED error TEXT improvable behind the registry; API churn
  recorded in `CHANGELOG.md ## [Unreleased]` same commit (running list
  retired — RELEASING.md updated); allocation removal needs criterion
  evidence; topology default KEEP.
- **Wave order after R1**: R2 decompose core (builder.rs/viewops.rs) + dsl
  (engine/mod.rs, stmt_views.rs); R3 export+layout+viewer (shell.rs,
  scene.rs, plantuml); R4 typed ids (export-heavy, 48 sites); R5 visibility
  shrink; R6 allocation removal (bench cmp mandatory); R7 topology verdict +
  idiom sweep; then v0.5.0 release close (bump, CHANGELOG cut, windows leg,
  **Category B mandatory**, dist plan, LOCAL tag).
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

- **P5R-R0a** (`75ce39d`): MSRV floor 1.88 + let-chain/as_chunks adoption
  (20 code files, clippy lint wave fixed same-commit), NIGHTLY row,
  CHANGELOG [Unreleased] seeded, pitfall note.
- **P5R-R0b** (this commit): PHASE_5R.md charter+waves, ERROR-MESSAGES.md
  provenance registry, PLAN/index rows, RELEASING churn convention,
  bench baseline `5R` (92 files, saved from the pre-P5R tree — R0a is
  codegen-neutral syntax only).
- Phase 4 complete: tag `v0.4.0` (local); M5f windows fix (`4e35ee2`).
  Bench baselines `1T`…`4T` committed.
