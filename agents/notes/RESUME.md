# RESUME — session checkpoint (updated 2026-11-28, **PHASE 7 IN FLIGHT — wave P7-M0 DONE (dagre adapter + V1 determinism gate GREEN + quality-metric harness + engine framework). NEXT = wave P7-M1 (dagre becomes the default engine) and then M2…M7. Previous arcs: P6 CLOSED 2026-11-28 (`9c4a4bf`…`41f9ddf`, no tag); P5R CLOSED 2026-11-26, tag `v0.5.0` LOCAL. Never pushed — push/publish stay the user's call**)

## Where P7 stands

- **M0 committed**: `dagre` 0.1.2 behind the NON-default `oxidizr-layout/dagre`
  feature; `Dagre` adapter at the `LayoutEngine` seam
  (`crates/oxidizr-layout/src/dagre.rs` — positional `n{i}`/`e{i}` names,
  center→top-left, multigraph+self-loops, origin-anchored, anchors ignored,
  non-finite guard ⇒ `LayoutError::Dagre`); viewer `Engine::Dagre` +
  `dagre` passthrough feature (dialog combo live, `Layered` STILL the
  default until M1).
- **V1 determinism gate GREEN**: `tests/dagre_determinism.rs` — 116 corpus
  variants × 3 in-process repeats + 10 concurrent fresh processes, bit-exact
  (digest `32baf4be2345c6bf`, ~16 s); `Layered` runs the same battery.
  No STOP needed (P7-D7).
- **V2 harness committed**: `tests/layout_quality.rs` (crossings/pierces/
  bends/length, TOTAL row, `expect!` table, re-bless = `UPDATE_EXPECT=1`).
  Measured LR/contract separations: TOTAL crossings 1503→1005, pierces
  304→191, bends 2974→2385. Gate = table + `KNOWN_CROSSING_REGRESSIONS =
  ["c4-like", "grid-120"]` + aggregate-better assertion (the "≥ Layered
  everywhere" wording was amended — 6 option profiles swept, none
  dominates; PHASE_7 §M0 measured reality + amended V2).
- **Two defects fixed on the way**: dagre 0.1.2 cannot build
  `default-features = false` (ungated `graph/json.rs` ⇒ `features = ["json"]`
  forced on; serde pair already in the viewer tree) — note
  `env/dagre-crate-json-feature.md`; and `oxidizr-layout --features
  dot-backend` was BROKEN since P5R-R5b (`e76ba12` left `pub(crate)` consts
  on the `pub use` line ⇒ E0364, no CI leg enabled the feature) — fixed by
  trimming the re-export to `pub use dot::Dot;`.
  Metric pitfall: `f64::signum(0.0) == 1.0` — note
  `pitfalls/f64-signum-zero-is-one.md`.
- User rulings anchoring everything (2026-11-28): dependency allowed ONLY
  crates.io + pure-Rust + statically linked + no multiplatform/compile
  burden; **dagre first, but the abstraction must keep multiple engines**
  (feature-enabled, GUI/CLI-selectable); **compound boundary layout**;
  **pin what I moved** (overlay positions = pins, exact); **per-view
  config + write-through** of the five standard `automaticLayout` fields;
  **no new external dependencies** — dot stays frozen inside the engine
  selection (no PATH discovery, no UX push).
- Work autonomously per the wave table; commit every green wave-piece;
  questions ONLY on plan-contradicting reality (AGENTS.md style).

## Implementation anchors (post-M0)

- `oxidizr-layout`: default features = ZERO deps (V8 measured, tree EMPTY);
  `--features dagre` = 14 crates, only `dagre` new vs the viewer tree, zero
  windowing. Seam = `engine.rs::LayoutEngine` (results in graph input
  order). `layered/` = own engine (M3 gains anchor clamps in the coordinate
  pass; M5 wires `edge_separation` through `layered/routing.rs` port spread
  — until M5 Layered IGNORES `edge_separation`, which flatters its
  `grid-120` metric numbers). Tests share `tests/common/mod.rs` (14-shape
  corpus, `Sweep::Full|Directions`, LCG fixtures, bit-exact encode + FNV
  digest) — extend THAT, not new fixture sets.
- Viewer (`crates/oxidizr-viewer/src/layout.rs`): `Engine` enum =
  Layered/Dagre(cfg)/Dot(cfg); `config_for`/`stored_for`/`display_params`
  (display floors rank sep at 200); `graph_for(scene, config)` maps
  items+edges ONLY — M2 adds boundary membership (scene `Boundary` carries
  members) as dagre compound parents; `apply`/`apply_with` → `Overrides`;
  `encode`/`decode` persist dialog params under `KEY_LAYOUT` (global) —
  M4 per-view with legacy fallback. Dialog `shell/actions.rs::layout_window`
  (combo lists `Engine::available()`); `run_layout` + `layout_engine` field
  in `shell/mod.rs`; manual positions = `LayoutOverlay` element entries =
  the M3 PIN source.
- M1 specifics: default switch = `Engine::default()` → `Dagre` **when the
  feature is compiled** (`#[cfg]` the `#[default]` — or make `available()`
  order the source of truth and seed `layout_engine` from
  `Engine::available()[0]`); auto-display uses `apply` → route it through
  the default engine too; scene-fixture snapshots (`golden_paint.rs`)
  re-bless ONLY where the metric improves; `bench cmp` recorded (dagre may
  be slower — record, don't hide).
- Write-back (M4): `ModelCommand::SetAutomaticLayout(view_key, from, to)`
  through `Viewer::run_model_edit` + gate `commands::model_editing_allowed`
  (plain `.json` only); undo restores prior `Option<AutomaticLayout>`;
  bless stays NO-OP.
- CLI (M6): `oxidizr view --layout-engine <layered|dagre|dot>` +
  completions; the CLI probably also wants a `dagre` passthrough feature
  (like viewer's) — decide there.

## If a next session opens

- Start wave **P7-M1** per the `build/PHASE_7.md` wave table (dagre default
  + metric re-check + bench cmp + snapshot policy V2). M0 evidence is in the
  plan §M0 measured reality; do not re-derive it.
- Feature-matrix sanity before commit (no CI leg covers these):
  `cargo check -p oxidizr-layout --features dot-backend,dagre --all-targets`
  and `cargo check -p oxidizr-viewer --features gui,dagre,dot-backend`.
- Gates: EVERY commit `cargo xtask ci gui fmt clippy test refs` — FULL
  form, every leg named; a bare `ci gui` runs ONE leg and still says
  "all steps green" (pitfall `xtask-ci-steps-replace-defaults.md`).
  Bless NO-OP every wave; bench budget changes recorded with numbers.
- Windows/Wine leg at close: dagre must cross-compile + run under Wine —
  it is pure Rust, but the leg is the proof (V7).
- Tags `v0.1.0…v0.5.0` all LOCAL; P6 unpunctuated by tag by design; P7
  closes WITHOUT a tag. Pushing stays the user's act.

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
  drafts go to the session tmp dir; apply after the chain, re-run `refs`.
- GUI-leg drift: `gui-leg-lint-drift.md` — the quartet never compiles the
  shell, so a plain `ci` passing does NOT mean the gui leg does.
- Shipped-text hygiene: bare F1..F19 / process ids in `crates/oxidizr-*/src`
  comments fail the self-gating xtask tests inside the `test`/`refs` legs
  (`refs-hygiene-prose-patterns.md`).

## Gates quick reference

- Sub-piece: `cargo xtask ci gui fmt clippy test refs` green → commit.
- Waves: + `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e` + bless NO-OP
  (307×unchanged, `git status --porcelain testdata/` empty) +
  `cargo doc --workspace --no-deps` at 0 (RUSTDOCFLAGS="-D warnings") +
  `bench cmp <LABEL>` (run `cargo xtask bench` in the same session first —
  P7 re-baselines when dagre changes the layout bench, with evidence).
- Category B env: `$OXIDZR_STRUCTURIZR_BIN=/nix/store/
  5yil7chk3cjg3qmsabq1vzdxv0v4bwm0-structurizr-2026.10.26/bin/structurizr`
  (dev shell exports it), legs `TZ=UTC`, background only. Close legs:
  `cargo xtask oracle verify` + `cargo xtask export-verify`.
- Windows leg: `cargo xtask ci windows` (Wine, dev shell; full suite, slow).
- `dist plan` = `nix develop --command dist plan` (not an xtask subcommand).
