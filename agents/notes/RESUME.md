# RESUME — session checkpoint (updated 2026-11-28, **PHASE 7 PLANNED — auto-layout quality (dagre) + configuration. Scope ratified via questions tool (two rounds); plan `build/PHASE_7.md` written; F15+D18 registered, O2 superseded. NEXT = wave P7-M0 (dagre spike + engine framework + determinism gate + quality-metric harness) and work autonomously through M0…M7. Previous arcs: P6 CLOSED 2026-11-28 (`9c4a4bf`…`41f9ddf`, no tag); P5R CLOSED 2026-11-26, tag `v0.5.0` LOCAL. Never pushed — push/publish stay the user's call**)

## Where P7 stands

- Plan: `agents/plan/build/PHASE_7.md` (Status PLANNED; decisions P7-D0…D7,
  waves M0…M7, verification V1…V8, risks). Registry row **F15**
  (`ADDITIONAL_FEATURES.md`); deviation **D18** (Apply pins manual
  placements vs the layout editor's relayout-everything); roadmap **O2**
  superseded in `PLAN.md`; board row PLANNED in `PROGRESS.md`.
- User rulings anchoring everything (2026-11-28): dependency allowed ONLY
  crates.io + pure-Rust + statically linked + no multiplatform/compile
  burden; **dagre first, but the abstraction must keep multiple engines**
  (feature-enabled, GUI/CLI-selectable); **compound boundary layout**;
  **pin what I moved** (overlay positions = pins, exact); **per-view
  config + write-through** of the five standard `automaticLayout` fields;
  **no new external dependencies** — dot stays frozen inside the engine
  selection (no PATH discovery, no UX push).
- Work autonomously per the wave table; commit every green wave-piece;
  questions ONLY on plan-contradicting reality (AGENTS.md style). **M0 is
  a HARD GATE: if dagre cannot pass the 10×-identical determinism property
  over the corpus (V1), the default switch (P7-D1) does not happen — stop
  and report with the evidence.**

## Implementation anchors (measured 2026-11-28, post-P6 tree)

- `oxidizr-layout`: ZERO-dep crate (`thiserror` hand-rolled error, P3-D1 —
  preserve for DEFAULT features; `dagre` goes behind a new NON-default
  `dagre` feature, mirroring how `dot-backend` was added). Seam =
  `engine.rs::LayoutEngine` (`layout(&LayoutGraph) -> LayoutResult`,
  results in graph input order = determinism contract). Graph types
  `graph.rs` (`LayoutGraph`/`Node`/`Edge`/`LayoutConfig`/`RankDirection`);
  `layered/` = own engine (cycle DFS, longest-path layering, dummy chains,
  barycenter sweeps with `crossings` metric, priority-method coordinates
  — THE pass that gains anchor clamps in M3); `layered/routing.rs` =
  port-spread fan (`edge_separation` wiring point, M5) + channel lanes;
  `dot.rs` behind `dot-backend` (env-var discovery only).
- `dagre` 0.1.2 (crates.io, Apache-2.0, edition 2024 ≥1.85 ≤ MSRV 1.88 —
  re-verify at M0): **verified signature**
  `dagre::layout(g: &mut Graph<NodeLabel, EdgeLabel>, opts: Option<LayoutOptions>)`
  — mutates in place (no Result), input per node `width`/`height` (per
  edge `minlen`/`weight`), output per node `x`/`y`/`rank`/`order`, per
  edge `points` (the waypoints). `Graph` (module `graph`) supports
  directed/multi/COMPOUND (parent/children); `LayoutOptions` = rankdir
  TB/BT/LR/RL, ranksep/nodesep/edgesep/…; ONLY mandatory dep `log`;
  serde/serde_json ride the `json` DEFAULT feature — **use
  `default-features = false`**. dagre semantics put node `x`/`y` at the
  CENTER (dagre.js parity) — the adapter converts to top-left and MUST
  re-verify against a fixture at M0. Output determinism UNCLAIMED —
  prove, then wrap.
- Viewer side (`crates/oxidizr-viewer/src/layout.rs`): `config_for` maps
  core `AutomaticLayout` → `LayoutConfig` (null direction = default LR);
  `stored_for`/`display_params` (display lower-bounds rank_separation at
  `DISPLAY_FALLBACK` = 200); `graph_for(scene, config)` maps items+edges
  ONLY — M2 adds boundary membership (scene `Boundary` already carries
  members) as dagre parents; `apply`/`apply_with(Engine, …)` →
  `Overrides`; `encode`/`decode` persist dialog params under `KEY_LAYOUT`
  (global) — M4 makes it per-view with legacy fallback. Dialog
  `shell/actions.rs::layout_window` (engine combo appears only when a
  second engine CAN run); `run_layout` + `layout_engine` field in
  `shell/mod.rs`; manual positions = `LayoutOverlay` element entries =
  the M3 PIN source.
- Write-back (M4): new `ModelCommand::SetAutomaticLayout(view_key, from,
  to)` through `Viewer::run_model_edit` + gate
  `commands::model_editing_allowed` (plain `.json` only — the P6 V7 matrix
  extends: `.dsl`, sidecar-preferred reopen, stdin all blocked); undo
  restores the prior `Option<AutomaticLayout>` (absent ⇒ remove). The
  JSON writer already round-trips the five `automaticLayout` fields —
  bless must stay NO-OP.
- CLI (M6): `oxidizr view --layout-engine <layered|dagre|dot>` via the
  existing clap surface + `completions` regenerate; flag validated as a
  value (no panics), cached choice.

## If a next session opens

- Start wave **P7-M0** per `build/PHASE_7.md` wave table: add `dagre`
  (default-features=false) behind `oxidizr-layout/dagre`; `Dagre` adapter
  at the seam; determinism property test (10× byte-identical, corpus incl.
  parallel edges + self-loops); quality-metric harness (crossings + bends
  + length, E-3 metric) with a committed corpus table; engine framework
  (`Engine::available()` + dialog combo + CLI flag skeleton); V4 tree
  check (+dagre +log headless, ZERO windowing crates; default-feature
  `cargo tree -e normal -p oxidizr-layout` still empty). **If V1 is red →
  STOP, report to the user with evidence** (P7-D7).
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
