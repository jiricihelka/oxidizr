# RESUME — session checkpoint (updated 2026-11-28, **PHASE 7 IN FLIGHT — waves P7-M0…P7-M4 DONE (dagre adapter + V1 gate; dagre DEFAULT engine; compound boundary layout; pinned tidy; per-view config + "Remember in view"). NEXT = wave P7-M5 (edge_separation into Layered port spread) and then M6…M7. Previous arcs: P6 CLOSED 2026-11-28 (`9c4a4bf`…`41f9ddf`, no tag); P5R CLOSED 2026-11-26, tag `v0.5.0` LOCAL. Never pushed — push/publish stay the user's call**)

## Where P7 stands

- **M4 committed (2026-11-28)**: per-view layout config + "Remember in view".
  - Command (`commands.rs`): `ModelCommand::SetAutomaticLayout { view_key,
    from: Option<AutomaticLayout>, to }` + factory `set_automatic_layout`
    — no command for a no-op/unknown-view/content-less key (no dead
    step); undo restores the PRIOR block or its ABSENCE; removal rides
    the same rail. Two tests: round-trips both directions + rejects;
    and the emitted view's `automaticLayout` carries EXACTLY the five
    standard JSON keys (extras impossible: the command carries the
    standard struct, the shared writer emits those five).
  - V5: the gate is the EXISTING `model_editing_allowed` matrix test
    (`.json` allowed; `.dsl`/stdin/sidecar-preferred blocked) — the
    command rides `run_model_edit` unchanged; the dialog DISABLES
    "Remember in view" with the reason on hover (`remember_in_view_row`
    in actions.rs, extracted to keep layout_window under the line
    budget).
  - Per-view cache (`shell/mod.rs`): session map `layout_by_view`,
    storage keys `KEY_LAYOUT/<view>`. MEASURED REALITY: eframe 0.37
    `Storage` has NO key enumeration and hands out the handle only in
    `startup`/`save` — per-view entries therefore read at STARTUP for
    the opened workspace's view keys; an in-session OPEN falls through
    stored→legacy→defaults. `save()` flushes the whole map AND keeps
    the legacy GLOBAL `KEY_LAYOUT` (fallback default, older-build
    compatible). Seed order (helper `seed_layout_params`): stored view
    block → per-view cache → legacy → defaults; called on every view
    switch (`show_view`), on `adopt`, and at startup. Apply caches
    per-view (`cache_layout_params`). ENGINE choice stays session-only.
- **M3 committed**: pinned tidy — overlay element entries ARE pins; a
  pinned Apply holds them EXACTLY (engine `Node.pin` + `BadPin`; anchor
  clamps in `layered/skeleton.rs`: priority pins > dummies > nodes,
  zero-line renormalisation skipped when pinned; `place()` emits the pin
  VERBATIM). `layout::apply_pinned` (empty map == `apply_with`
  byte-identical; non-empty forces Layered). `TogglePin` + Shift-click
  while the dialog is open + pin badge + "Apply (ignore pins)".
- **M2 committed**: compound boundary layout (group forest seam + dagre
  compound + scene membership; V1 green on 134 corpus variants; group
  rows 0 cross/0 pierce).
- **M1 committed**: dagre DEFAULT engine; quality TOTAL cross
  1503→1005, pierce 304→191, bend 2974→498.
- **M0 committed**: `dagre` 0.1.2 behind the NON-default `dagre`
  feature; V1 determinism gate GREEN; V2 quality harness + blessed
  table (`KNOWN_CROSSING_REGRESSIONS = ["c4-like", "grid-120"]`).
- User rulings anchoring everything (2026-11-28): dependency allowed
  ONLY crates.io + pure-Rust + statically linked + no multiplatform/
  compile burden; dagre first but KEEP multiple engines; compound
  layout (done); pin what I moved (done); per-view config + write-
  through of the five standard fields (done); NO new external deps —
  dot stays frozen inside the engine selection.
- Work autonomously per the wave table; commit every green wave-piece;
  questions ONLY on plan-contradicting reality.

## Implementation anchors (post-M4)

- **M5 = the edge_separation wave** (P7-D6 + wave row): wire
  `edge_separation` into the Layered PORT SPREAD in
  `crates/oxidizr-layout/src/layered/routing.rs` (Layered IGNORES it
  today; dagre already honours it via `edgesep`). Check how the viewer
  passes params (`layout.rs::apply_with/apply_pinned` → the engine's
  params struct) and that the Layered path threads edge_separation
  where ports are spread. Re-check the dialog's fallback params
  against dagre's effective defaults. Wave row demands a
  **metric-gated re-bless (V2)**: the quality table MAY move for the
  Layered rows, dagre rows must stay byte-unchanged, and the harness
  gates (no new crossing regressions, TOTAL improves) decide. Layered
  unit tests asserting exact coordinates get updated WITH evidence,
  never to silence a gate. Dialog/labels must be honest afterwards
  (the stale "parity field only" wording is already gone from the
  dialog doc — re-scan for stragglers, incl. the layout.rs doc
  comments).
- `oxidizr-layout`: default features = ZERO deps; `--features dagre`.
  Seam = `engine.rs::LayoutEngine` (results in graph input order).
  `layered/` = own engine (coordinate pass carries the M3 pin clamps).
  Tests share `tests/common/mod.rs` (17-shape corpus, Sweep::Full|
  Directions, LCG fixtures, bit-exact encode + FNV digest) — extend
  THAT, not new fixture sets.
- Viewer: `layout.rs` `Engine` enum (Layered/Dagre(cfg)/Dot(cfg)),
  `graph_for` = nodes + edges + boundary FOREST, `apply`/`apply_with`/
  `apply_pinned` → `Overrides`, `encode`/`decode` persist dialog
  params. Shell: `run_layout(params, ignore_pins)`; pins =
  `LayoutOverlay.elements`; Shift-click pin toggle while the dialog is
  open; per-view param cache + `seed_layout_params` (M4).
- dagre source audit: `/tmp/opencode/dagre-src/dagre-0.1.2/`
  (re-unpack from the cargo registry cache if gone).
- CLI (M6): `oxidizr view --layout-engine <layered|dagre|dot>` +
  completions; decide whether the CLI gets a `dagre` passthrough
  feature (viewer already has one); viewer README + dialog polish +
  CHANGELOG review + manual smoke lists into the wave rows.
- M7 close: full V-list, `cargo xtask ci windows` (V7: dagre must run
  under Wine — pure Rust, the leg is the proof), oracle + export-verify
  with `TZ=UTC`, bench, bless NO-OP, NO tag, no push.

## If a next session opens

- Start wave **P7-M5** per the `build/PHASE_7.md` wave table. M4
  evidence is in the plan row; do not re-derive it.
- Feature-matrix sanity before commit (no CI leg covers these):
  `cargo clippy -p oxidizr-layout --features dot-backend,dagre --all-targets`
  and `cargo clippy -p oxidizr-viewer --features gui,dagre,dot-backend --all-targets`.
- Gates: EVERY commit `cargo xtask ci gui fmt clippy test refs` — FULL
  form, every leg named; a bare `ci gui` runs ONE leg and still says
  "all steps green" (pitfall `xtask-ci-steps-replace-defaults.md`).
  Bless NO-OP every wave EXCEPT the M5 re-bless (metric-gated, with
  evidence in the wave row).
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
  + `bench cmp <LABEL>` (run `cargo xtask bench` in the same session
  first — P7 re-baselines when a wave changes the layout bench, with
  evidence).
- Category B env: `$OXIDZR_STRUCTURIZR_BIN=/nix/store/
  5yil7chk3cjg3qmsabq1vzdxv0v4bwm0-structurizr-2026.10.26/bin/structurizr`
  (dev shell exports it), legs `TZ=UTC`, background only. Close legs:
  `cargo xtask oracle verify` + `cargo xtask export-verify`.
- Windows leg: `cargo xtask ci windows` (Wine, dev shell; full suite,
  slow).
- `dist plan` = `nix develop --command dist plan` (not an xtask
  subcommand).
