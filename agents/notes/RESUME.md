# RESUME — session checkpoint (updated 2026-11-26, **PHASE 5R CLOSED — tag `v0.5.0` LOCAL (annotated, on the close commit); tree clean, nothing pending. Never pushed — push/publish stay the user's call. Previous arcs: P4 → `v0.4.0`, P3 → `v0.3.0`**

## State at close

- **P5R = the large-scale idiomatic-Rust refactor**, waves R0…R7 + release
  close, all green per the evidence rows in `agents/plan/build/PHASE_5R.md`
  (Status: CLOSED) and the PROGRESS milestone-log entry. Commits
  `75ce39d` (R0a) → close; the release ritual followed `docs/RELEASING.md`
  end-to-end on the final tree (quartet+gui, lint, full tier, bless
  307×unchanged NO-OP, doc-zero, `dist plan` EXIT 0, `ci windows` green,
  Category B: oracle 4 json + 4 sidecar + 99 diagram + 6 theme, export-verify
  31/31 ×3 TZ=UTC).
- **Two REAL bugs were found by the windows leg and fixed (`a48b306`)**:
  fmt swallowed content after a continuation-joined comment ending in a
  backslash (proptest regression replay = cross-platform proof; never delete
  a regression line — it is evidence); `!docs` filename trim hardcoded `/`
  where the tooling checks `File.separator` (Linux bytes identical after the
  fix, bless-proved). The Java source at the pinned monorepo commit decided
  this — fetched `DefaultDocumentationImporter.java`, read it, don't guess.
- Bench: baseline `5R` in `bench-baseline/` remains the P5R reference;
  `bench cmp 5R` at R6 close showed every leg ≤ 1.01 with the wave targets
  faster. The NEXT phase starts by `bench save <newlabel>` if it wants a new
  reference (never mid-phase).

## If a next session opens

- No open tasks. A new phase needs a `agents/plan/build/PHASE_N.md` with the
  charter/decision-register pattern (see PHASE_5R.md as template) and user
  sign-off for scope; post-release backlog candidates only:
  - Windows target support is now green in CI's sense (full suite via Wine
    locally, `cargo xtask ci windows`), but NEW filesystem tests must run
    that leg EARLY (pitfall `windows-path-separator-assertions.md` — three
    trap shapes now documented there, incl. canonicalize-verbatim vs the
    lexical `getAbsolutePath` port).
  - Registry publishing remains explicitly skipped (RELEASING.md decision
    row) unless the user opens it.
- Tags `v0.1.0…v0.5.0` all LOCAL — pushing them is the user's act; CI then
  builds/publishes per `.github/workflows/release.yml`.

## Workflow (measured, `agents/notes/process/parallelism.md`)

- NO subagents by default (measured memory thrash; hard cap 3). Parallelize
  only harness cycles via background shell jobs. Two cargo jobs queue on the
  target-dir lock — chain in one background command; don't poll background
  shells (`background: true` as a BOOLEAN, string "True" is rejected).
- Script hygiene: python to a file under the session tmp dir, never
  `python3 -c` with backticks/`\b` (pitfall `agent-shell-quoting.md`); regex
  replaces MUST use f-strings for `{name}`; the nix dev-shell banner prefixes
  redirected stdout — strip to the first `{` before JSON parsing; `od`
  slicing must account for it (it ate a fmt-bug repro once).
- Gate-log reading: `grep -c` exits 1 on zero matches — printed `0`/
  `FAILED 0` is GREEN; verify `ci: all steps green` in the log body.
- Never edit tracked files while a gate/bench/bless chain runs; journal
  drafts go to the session tmp dir (`/tmp/opencode/p5r-close-stamps.md` was
  this close's pattern — apply after the chain, then re-run `refs`).
- `sed` pin sweeps must allow digits in crate names (`e2e`); a missed pin
  surfaces as a `cargo update` resolution error, not silently.

## Gates quick reference

- Sub-piece: quartet green → commit (`cargo xtask ci` in `nix develop`).
- Viewer/cli-gui/dsl-touching commits: `cargo xtask ci gui fmt clippy test refs`.
- Waves: + `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e` + bless NO-OP
  (307×unchanged, `git status --porcelain testdata/` empty) +
  `cargo doc --workspace --no-deps` at 0 + `bench cmp <LABEL>` (run
  `cargo xtask bench` in the same session first).
- Category B env: `$OXIDZR_STRUCTURIZR_BIN=/nix/store/
  5yil7chk3cjg3qmsabq1vzdxv0v4bwm0-structurizr-2026.10.26/bin/structurizr`
  (dev shell exports it), legs `TZ=UTC`, background only.
- Windows leg: `cargo xtask ci windows` (Wine, dev shell; full suite, slow —
  for triage use `cargo test -p <crate> --target x86_64-pc-windows-gnu
  --no-fail-fast`).
- `dist plan` = `nix develop --command dist plan` (cargo-dist binary; NOT an
  xtask subcommand).
- Pitfalls to honor: `windows-path-separator-assertions.md` (now 3 shapes +
  Wine lessons), `gui-leg-lint-drift.md`, `msrv-bump-clippy-wave.md`,
  `bench-machine-drift-ab.md`, `cargo-dist-installer-panic.md`.
