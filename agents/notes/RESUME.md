# RESUME — session checkpoint (updated 2026-11-28, **PHASE 7 — waves P7-M0…P7-M6 DONE. LAST WAVE = P7-M7 (close: full V-list + journals; NO tag, no push). Previous arcs: P6 CLOSED 2026-11-28 (`9c4a4bf`…`41f9ddf`, no tag); P5R CLOSED 2026-11-26, tag `v0.5.0` LOCAL. Never pushed — push/publish stay the user's call**

## Where P7 stands

- **M6 committed (2026-11-28)**: `oxidizr view --layout-engine
  <layered|dagre|dot>` on BOTH entries (cli mirror + viewer binary)
  through the ONE `shell::run(file, theme, engine)` seam;
  `LayoutEngineArg` ValueEnum in shell; unavailable request →
  default kept + dialog/status note (errors as values). Flag-name
  flattening pitfall (`--layoutengine`) fixed with explicit
  `long = "layout-engine"` on both surfaces. Completions render the
  flag (clap_complete; scripts green). No cli dagre passthrough
  feature needed — viewer default features ride in. Docs: CHANGELOG
  Unreleased + viewer README + shortcuts pin row; smoke lists M1–M5
  in the wave rows.
- **M5 committed**: `edge_separation` honoured by BOTH engines —
  Layered port spread places fans CENTRED at exact spacing when
  `(N−1)·edge_sep ≤ side_len`, equal-slot fallback otherwise, N = 1
  byte-unchanged. Metric-gated V2 re-bless GREEN (Layered improves,
  dagre columns byte-unchanged, verdict set unchanged).
- **M4 committed**: per-view layout config (`layout_by_view`,
  `KEY_LAYOUT/<view>`, legacy fallback, seed order stored→cache→
  legacy→defaults) + `ModelCommand::SetAutomaticLayout`
  ("Remember in view", five-standard-fields write-back, undoable).
- **M3 committed**: pinned tidy — overlay entries ARE pins, exact
  hold via anchor clamps, `apply_pinned`, `TogglePin` + Shift-click
  while the dialog is open + "Apply (ignore pins)".
- **M2 committed**: compound boundary layout (dagre compound; V1
  green on 134 corpus variants).
- **M1 committed**: dagre DEFAULT engine (quality TOTAL cross
  1503→1005, pierce 304→191, bend 2974→498).
- **M0 committed**: `dagre` 0.1.2 behind the NON-default `dagre`
  feature; V1 determinism GREEN; V2 harness + blessed table.
- User rulings anchoring everything (2026-11-28): crates.io +
  pure-Rust + static + no multiplatform burden ONLY; dagre first,
  KEEP multiple engines; compound layout; pin what I moved; per-view
  config + five-field write-through; NO new external deps — dot
  frozen inside the engine selection.

## P7-M7 — the close wave (what remains)

Per the `build/PHASE_7.md` wave table, M7 = close: journals + full
V-list. Checklist with commands:

1. **Full tier**: `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e`
   (background — slow).
2. **Doc-zero**: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace
   --no-deps` at 0 warnings.
3. **Windows leg**: `cargo xtask ci windows` (Wine, dev shell:
   `nix develop --command cargo xtask ci windows`; V7 proof = dagre
   pure Rust cross-compiles AND runs under Wine). Slow — background.
4. **Category B**: `cargo xtask oracle verify` and
   `cargo xtask export-verify` with `TZ=UTC` and
   `$OXIDZR_STRUCTURIZR_BIN=/nix/store/5yil7chk3cjg3qmsabq1vzdxv0v4bwm0-structurizr-2026.10.26/bin/structurizr`
   (the dev shell exports it; run inside `nix develop --command …`,
   legs `TZ=UTC`, background).
5. **Bench**: `cargo xtask bench` (posture: R-5 budgets stand — the
   M5 change is inside the O(E) spread pass; record numbers vs M1).
6. **Bless NO-OP**: `git status --porcelain testdata/` empty.
7. **Journals**: PHASE_7 M7 row DONE + the phase-close stamp,
   `agents/plan/PROGRESS.md` P7 entry (milestone log), this RESUME
   rewritten "P7 CLOSED".
8. **NO tag** (P7 closes unpunctuated by design), **no push**.

Run the slow legs as ONE background chain where cargo's lock allows,
or sequentially in background; never poll — wait for notifications.
Order suggestion: (2) quick first, then (1); then (3) windows leg;
then (4) oracle legs; then (5); journals + final commit last, after
every leg's evidence exists.

## Implementation anchors (post-M6)

- `oxidizr-layout`: default features = ZERO deps; `--features dagre`.
  `layered/routing.rs::port_spread` takes the separation; the
  quality table (`tests/layout_quality.rs`) is the blessed metric
  harness (`UPDATE_EXPECT=1 cargo test -p oxidizr-layout
  --all-features --test layout_quality` re-blesses; V2 gates: dagre
  columns byte-unchanged, verdict set unchanged, TOTAL improves).
- Viewer: `shell::run(file, ThemeArg, Option<LayoutEngineArg>)` —
  the ONE entry; `layout::Engine` (Layered / Dagre-cfg / Dot-cfg),
  `apply`/`apply_with`/`apply_pinned`, `encode`/`decode` five-field
  format (engine NOT persisted); per-view params
  (`layout_by_view` + `seed_layout_params`/`cache_layout_params`).
- CLI: `Command::View { file, theme, layout_engine }` —
  `ViewLayoutEngine` mirrors the shell's enum; `completions` renders
  the argv surface via clap_complete (tests/cli.rs pins entry lines
  only).
- dagre source audit: `/tmp/opencode/dagre-src/dagre-0.1.2/`
  (crate defaults 50/20/50 at `layout/types.rs:29-31` — adapter
  always passes dialog values, so they are moot).

## If a next session opens

- Wave **P7-M7** per the checklist above; M0–M6 evidence lives in the
  `build/PHASE_7.md` wave rows — do not re-derive it.
- Gates: EVERY commit `cargo xtask ci gui fmt clippy test refs` —
  FULL form, every leg named (pitfall
  `xtask-ci-steps-replace-defaults.md`). Bless NO-OP (`git status
  --porcelain testdata/` empty).
- Feature-matrix sanity before commit:
  `cargo clippy -p oxidizr-layout --features dot-backend,dagre --all-targets`
  and `cargo clippy -p oxidizr-viewer --features gui,dagre,dot-backend --all-targets`.
- Multi-line commit messages: `git commit -F <file>` — inline `-m`
  with `\n` produces literal backslash-n (pitfall hit at M4).
- Shipped-prose hygiene: milestone ids in `crates/oxidizr-*/src`
  comments FAIL `test`+`refs` (note `refs-hygiene-prose-patterns.md`).
- Tags `v0.1.0…v0.5.0` all LOCAL; P6 and P7 close WITHOUT tags by
  design. Pushing stays the user's act.

## Workflow (measured, `agents/notes/process/parallelism.md`)

- NO subagents by default (measured memory thrash; hard cap 3).
  Parallelize only harness cycles via background shell jobs. Two cargo
  jobs queue on the target-dir lock — chain in one background command;
  don't poll background shells (`background: true` as a BOOLEAN).
- Script hygiene: python to a file under the session tmp dir, never
  `python3 -c` with backticks/`\b` (pitfall `agent-shell-quoting.md`);
  the nix dev-shell banner prefixes redirected stdout — strip to the
  first `{` before JSON parsing.
- Gate-log reading: `grep -c` exits 1 on zero matches — printed `0`/
  `FAILED 0` is GREEN; verify `ci: all steps green` in the log body.
- Never edit tracked files while a gate/bench/bless chain runs; journal
  drafts go to the session tmp dir; apply after the chain, re-run `refs`.
- GUI-leg drift: `gui-leg-lint-drift.md` — the quartet never compiles
  the shell, so a plain `ci` passing does NOT mean the gui leg does.
- Clap flag names: a doc comment alone does NOT stop snake_case
  flattening (`layout_engine` → `--layoutengine`); pass an explicit
  `long = "…"` (precedent `--ignore-layout`; hit again at P7-M6).

## Gates quick reference

- Sub-piece: `cargo xtask ci gui fmt clippy test refs` green → commit.
- Waves: + `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e` + bless
  NO-OP + `cargo doc --workspace --no-deps` at 0
  (RUSTDOCFLAGS="-D warnings") + bench posture recorded.
- Category B env: `$OXIDZR_STRUCTURIZR_BIN=/nix/store/
  5yil7chk3cjg3qmsabq1vzdxv0v4bwm0-structurizr-2026.10.26/bin/structurizr`
  (dev shell exports it), legs `TZ=UTC`, background only. Close legs:
  `cargo xtask oracle verify` + `cargo xtask export-verify`.
- Windows leg: `cargo xtask ci windows` (Wine, dev shell; full suite,
  slow).
- `dist plan` = `nix develop --command dist plan` (not an xtask
  subcommand).
