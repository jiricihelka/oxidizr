# RESUME — session checkpoint (updated 2026-11-28, **PHASE 7 IN FLIGHT — waves P7-M0…P7-M5 DONE (dagre adapter + V1 gate; dagre DEFAULT engine; compound boundary layout; pinned tidy; per-view config + "Remember in view"; edge_separation wired into Layered). NEXT = wave P7-M6 (CLI --layout-engine + docs polish + manual smoke) and then M7 close. Previous arcs: P6 CLOSED 2026-11-28 (`9c4a4bf`…`41f9ddf`, no tag); P5R CLOSED 2026-11-26, tag `v0.5.0` LOCAL. Never pushed — push/publish stay the user's call**

## Where P7 stands

- **M5 committed (2026-11-28)**: `edge_separation` honoured by BOTH
  engines. Layered port spread now places a (node, side) fan CENTRED
  with EXACT spacing when `edge_sep > 0` and `(N−1)·edge_sep ≤
  side_len` (side = extent along the slide axis); zero/negative or a
  fan that wouldn't fit falls back to the old N+1 equal slots; N = 1
  keeps the midpoint byte-identically (single-edge groups skipped).
  Call site: `skeleton.rs` `place()` passes
  `f64::from(graph.config.edge_separation)`. Tests: 2 old-fraction
  tests updated WITH EVIDENCE (y 80→95; renamed
  `edges_sharing_a_side_spread_by_edge_separation`), 2 NEW tests
  (exact 15 px steps + bit-exact re-run; crowded-side fallback).
  Metric-gated V2 re-bless GREEN: Layered rows only improve (TOTAL
  cross 1503→1490, pierce 304→293, bend 3006→3002, len 677 553),
  dagre columns BYTE-UNCHANGED, verdict set unchanged (c4-like +
  grid-120 still WORSE). Display fallback re-check: dagre crate
  defaults 50/20/50 are irrelevant (adapter passes dialog values
  explicitly) — DISPLAY_FALLBACK 200/120/50 stays, edge 50 == JSON
  default. Honesty docs updated in viewer `layout.rs` + harness note
  + `port_spread` doc.
- **M4 committed**: per-view layout config + "Remember in view"
  (`ModelCommand::SetAutomaticLayout` + factory, five-standard-fields
  write-back test, `layout_by_view` cache under `KEY_LAYOUT/<view>` +
  legacy GLOBAL fallback; eframe 0.37 Storage reads per-view entries
  at STARTUP only — no key enumeration, measured reality). Seed order
  stored block → cache → legacy → defaults; `seed_layout_params` /
  `cache_layout_params`; engine choice session-only.
- **M3 committed**: pinned tidy — overlay entries ARE pins, exact hold
  via anchor clamps, `apply_pinned`, `TogglePin` + Shift-click +
  "Apply (ignore pins)".
- **M2 committed**: compound boundary layout (group forest seam + dagre
  compound + scene membership; V1 green on 134 corpus variants).
- **M1 committed**: dagre DEFAULT engine; quality TOTAL cross
  1503→1005, pierce 304→191, bend 2974→498.
- **M0 committed**: `dagre` 0.1.2 behind the NON-default `dagre`
  feature; V1 determinism gate GREEN; V2 quality harness + blessed
  table (`KNOWN_CROSSING_REGRESSIONS = ["c4-like", "grid-120"]`).
- User rulings anchoring everything (2026-11-28): dependency allowed
  ONLY crates.io + pure-Rust + statically linked + no multiplatform/
  compile burden; dagre first but KEEP multiple engines; compound
  layout (done); pin what I moved (done); per-view config + write-
  through of the five standard fields (done); edge separation honest
  (done); NO new external deps — dot stays frozen inside the engine
  selection.
- Work autonomously per the wave table; commit every green wave-piece;
  questions ONLY on plan-contradicting reality.

## Implementation anchors (post-M5)

- **M6 = CLI + docs wave** (wave row: "CLI `oxidizr view
  --layout-engine <layered|dagre|dot>` + completions; CLI dagre
  passthrough feature decision; viewer README + dialog polish;
  CHANGELOG review; manual smoke lists into the wave rows").
  - CLI: `crates/oxidizr-cli` — find the `view` subcommand and its
    clap args; the viewer lib's `layout::Engine` is gui-feature-gated
    in the SHELL, but `oxidizr-layout`'s engine seam is headless — the
    CLI's headless layout path (if `view` renders headless, check what
    engine it uses today) gets a `--layout-engine` flag. Decide the
    passthrough feature: viewer already defaults `dagre`; the CLI
    should probably MIRROR that (default-on `dagre` feature in
    oxidizr-cli, so `--layout-engine dagre` works out of the box) —
    record the decision with rationale (pure Rust, static, no burden —
    same argument as the viewer's flip).
  - Completions: the CLI has `completions` (check
    `crates/oxidizr-cli/src/main.rs` / `cli.rs` clap setup + any
    `clap_complete`); a new enum arg needs only the variant list —
    regenerate/verify the completion test if one exists.
  - Manual smoke lists: the plan wants the manual GUI smoke steps
    written into the wave rows (M3/M4/M5 dialogs: pin Shift-click,
    Remember in view, separation fan) — add a short `SMOKE` section
    per wave or one consolidated list in the M6 row.
  - Viewer README (`crates/oxidizr-viewer/README.md`): engines section
    (dagre default, Layered fallback, dot opt-in), dialog fields now
    honest (edge separation real for both), per-view config +
    Remember-in-view, pins.
  - CHANGELOG review: P7 entries through the wave commits — check
    `CHANGELOG.md` exists and its Keep-a-Changelog shape; add/adjust.
- **M7 close** (wave row): journals (PHASE_7/PROGRESS/RESUME), full
  V-list — full tier (`OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e`),
  doc-zero (`RUSTDOCFLAGS="-D warnings" cargo doc --workspace
  --no-deps`), `cargo xtask ci windows` (Wine, dev shell — V7 dagre
  cross-compile proof), Category B oracle + export-verify (`TZ=UTC`,
  `$OXIDZR_STRUCTURIZR_BIN` from dev shell), bench, bless NO-OP,
  NO tag, no push.
- `oxidizr-layout`: default features = ZERO deps; `--features dagre`.
  Seam = `engine.rs::LayoutEngine`. `layered/` = own engine
  (`routing::port_spread` now takes the separation; `skeleton.rs`
  threads `graph.config.edge_separation`). Tests share
  `tests/common/mod.rs` — extend THAT, not new fixture sets.
- Viewer: `layout.rs` `Engine` enum, `graph_for` forest,
  `apply`/`apply_with`/`apply_pinned`, `encode`/`decode` (five-field
  format unchanged — engine NOT persisted); shell `run_layout`,
  `layout_by_view`/`layout_legacy` + `seed_layout_params`/
  `cache_layout_params` (M4).
- dagre source audit: `/tmp/opencode/dagre-src/dagre-0.1.2/`
  (re-unpack from the cargo registry cache if gone).

## If a next session opens

- Start wave **P7-M6** per the `build/PHASE_7.md` wave table; M5
  evidence is in the plan row; do not re-derive it.
- Feature-matrix sanity before commit (no CI leg covers these):
  `cargo clippy -p oxidizr-layout --features dot-backend,dagre --all-targets`
  and `cargo clippy -p oxidizr-viewer --features gui,dagre,dot-backend --all-targets`.
  After M6 add the CLI matrix (`oxidizr-cli gui` + dagre, whatever the
  passthrough decision is).
- Gates: EVERY commit `cargo xtask ci gui fmt clippy test refs` — FULL
  form, every leg named; a bare `ci gui` runs ONE leg and still says
  "all steps green" (pitfall `xtask-ci-steps-replace-defaults.md`).
  Bless NO-OP every wave (M5's metric-gated re-bless was the only
  exception and it is DONE).
- Multi-line commit messages: write to a file, `git commit -F <file>`
  — inline `-m` with `\n` produces literal backslash-n (pitfall hit at
  M4, fixed by amend).
- Shipped-prose hygiene: milestone ids (`§M2`, `P7-M2`) in
  `crates/oxidizr-*/src` comments FAIL the `test`+`refs` legs (note
  `refs-hygiene-prose-patterns.md`). Kinds/roles in prose, process ids
  only in `agents/`.
- Tags `v0.1.0…v0.5.0` all LOCAL; P6 unpunctuated by tag by design; P7
  closes WITHOUT a tag. Pushing stays the user's act.

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

## Gates quick reference

- Sub-piece: `cargo xtask ci gui fmt clippy test refs` green → commit.
- Waves: + `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e` + bless
  NO-OP (`git status --porcelain testdata/` empty) +
  `cargo doc --workspace --no-deps` at 0 (RUSTDOCFLAGS="-D warnings")
  + bench posture recorded (M5: metric table is the evidence, R-5
  budgets stand).
- Category B env: `$OXIDZR_STRUCTURIZR_BIN=/nix/store/
  5yil7chk3cjg3qmsabq1vzdxv0v4bwm0-structurizr-2026.10.26/bin/structurizr`
  (dev shell exports it), legs `TZ=UTC`, background only. Close legs:
  `cargo xtask oracle verify` + `cargo xtask export-verify`.
- Windows leg: `cargo xtask ci windows` (Wine, dev shell; full suite,
  slow).
- `dist plan` = `nix develop --command dist plan` (not an xtask
  subcommand).
