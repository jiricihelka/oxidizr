# RESUME — session checkpoint (updated 2026-11-28, **PHASE 7 IN FLIGHT — waves P7-M0…P7-M3 DONE (dagre adapter + V1 gate; dagre DEFAULT engine; compound boundary layout; pinned tidy with anchor clamps). NEXT = wave P7-M4 (per-view config + "Remember in view") and then M5…M7. Previous arcs: P6 CLOSED 2026-11-28 (`9c4a4bf`…`41f9ddf`, no tag); P5R CLOSED 2026-11-26, tag `v0.5.0` LOCAL. Never pushed — push/publish stay the user's call**)

## Where P7 stands

- **M3 committed (2026-11-28)**: pinned tidy — the overlay's element
  entries ARE pins, a pinned Apply holds them EXACTLY.
  - Engine seam (`oxidizr-layout`): `Node.pin: Option<Point>` +
    `LayoutError::BadPin` (validate finiteness). Anchor clamps live in
    `layered/skeleton.rs`: `priority(verts, node_sep, pins)` seeds pinned
    vertices at their mapped positions, immovable (rank order + node_sep
    clamp the live centres; priority pins > dummies > nodes); the
    zero-line renormalisation is SKIPPED when any pin exists (pins ARE
    the frame). `place()` maps pins INVERSELY to canonical coords
    (all four directions), emits `pin` VERBATIM (the RL/BT mirror is a
    subtraction — not round-trip bit-exact — the pin is the contract)
    and takes pinned route-centres from the EMITTED boxes. Dagre/Dot
    never read `pin` (`pins_do_not_reach_the_compound_engine`).
  - Viewer seam (`layout.rs::apply_pinned(engine, scene, layout, pins)`):
    empty map == `apply_with` byte-identical (the escape hatch runs
    exactly what the engine alone would); non-empty forces Layered.
    Shell `run_layout(params, ignore_pins)`; pins = `overlay.elements`.
  - Command `TogglePin { id, from, to }` + `commands::toggle_pin`
    factory (pin at the CURRENT rendered rect; unpin drops the entry so
    stored geometry re-appears), round-trip test. GUI: Shift-click
    toggles a pin ONLY while the layout dialog is open (elsewhere Shift
    keeps multi-select); corner-dot badge (`paint::PIN_COLOR`,
    `PaintOptions.pinned`, `pinned_ids` helper) while the dialog is
    open; dialog shows pin count + engine-switch label + "Apply (ignore
    pins)".
  - V4: `tests/pinned_layout.rs` (big-bank-plc Components — engine-held
    + user-dragged pins byte-exact through override→scene→re-Apply,
    re-Apply bit-identical). V2 holds: quality table BYTE-UNCHANGED,
    un-pinned Layered path byte-identical
    (`unpinned_run_is_byte_identical_to_the_same_run_without_pins`),
    bless NO-OP.
- **M2 committed**: compound boundary layout end to end.
  - Seam (`oxidizr-layout`): `LayoutGraph.groups: Vec<Group>`
    (`Group { id: GroupId, parent: Option<GroupId>, members }` — DIRECT
    members, containment is a forest) + `add_group` + five `validate`
    invariants (`DuplicateGroupId`/`UnknownGroupMember`/`UnknownGroupParent`
    /`DuplicateGroupMembership`/`GroupParentCycle` — bounded chain walk).
    `Dot` NEVER reads `groups` (byte-identity test pins the V2 baseline;
    since M3 `Layered` reads `pin` but not `groups`).
  - Adapter (`dagre.rs`): compound ON only when a group is ACTIVE (has
    members or an active child — `active_groups`); parents = zero-size
    `g{bi}` nodes (dagre re-derives frames from border dummies), links via
    `set_parent`, nesting chains set last, parents never in the output.
    All-empty groups ⇒ byte-identical to no groups (`inactive_groups_
    change_nothing`).
  - Scene: `Boundary.members: Vec<Id>` (dn closure memoized beside
    `dn_rect` — `dn_member_items`); `boundary_from` takes an `Identity`
    bundle (8-arg form tripped the pedantic budget); `BOUNDARY_PADDING`
    is now `pub` (the V3 test asserts frames against it).
  - Forest (`layout.rs::graph_for` → `emit_groups`): item owner =
    SMALLEST containing boundary (fewer members → kind rank Group <
    DeploymentNode < Container < SoftwareSystem → scene order); parent =
    smallest STRICT member-set superset (strictness ⇒ acyclic).
  - Verification: V1 GREEN on 134 corpus variants (3 group fixtures added
    to `tests/common/mod.rs`); quality table re-blessed — old rows
    byte-unchanged (Layered untouched), group rows 0 cross/0 pierce,
    bends 6→3, 6→2, 20→10 (TOTAL bends 3006→513); V3 = unit forest tests
    + `tests/grouped_layout.rs` corpus sweep (frames = member unions +
    pad, edges attached, re-Apply byte-identical).
- **M1 committed**: dagre DEFAULT engine (`default = ["dagre"]`,
  hand-written `Engine::default()`, `simplify()` collinear drop). Quality
  TOTAL cross 1503→1005, pierce 304→191, bend 2974→498. Benches recorded
  (`benches/dagre.rs` sample 20): dagre 5.75 s vs Layered 4.45 ms mixed-1k.
- **M0 committed**: `dagre` 0.1.2 behind the NON-default `dagre` feature
  (`features = ["json"]` forced — note `env/dagre-crate-json-feature.md`);
  `Dagre` adapter; V1 determinism gate GREEN; V2 harness = blessed table
  + `KNOWN_CROSSING_REGRESSIONS = ["c4-like", "grid-120"]` + TOTAL improves.
- User rulings anchoring everything (2026-11-28): dependency allowed ONLY
  crates.io + pure-Rust + statically linked + no multiplatform/compile
  burden; **dagre first, but the abstraction must keep multiple engines**;
  **compound boundary layout** (M2 — done); **pin what I moved** (overlay
  positions = pins, exact — M3 — done); **per-view config + write-through**
  of the five standard `automaticLayout` fields (M4); **no new external
  dependencies** — dot stays frozen inside the engine selection.
- Work autonomously per the wave table; commit every green wave-piece;
  questions ONLY on plan-contradicting reality (AGENTS.md style).

## Implementation anchors (post-M3)

- `oxidizr-layout`: default features = ZERO deps; `--features dagre` =
  dagre crate. Seam = `engine.rs::LayoutEngine` (results in graph input
  order). `layered/` = own engine — the coordinate pass carries the pin
  clamps (M3); **M5 wires `edge_separation` through `layered/routing.rs`
  port spread** (until M5 Layered IGNORES `edge_separation`). Tests share
  `tests/common/mod.rs` (17-shape corpus incl. `grouped-chains`/
  `groups-nested`/`c4-grouped`, `Sweep::Full|Directions`, LCG fixtures,
  bit-exact encode + FNV digest) — extend THAT, not new fixture sets.
- Viewer (`crates/oxidizr-viewer/src/layout.rs`): `Engine` enum =
  Layered/Dagre(cfg)/Dot(cfg), `Engine::default()` feature-aware;
  `graph_for` = nodes + edges + the boundary FOREST; `apply`/`apply_with`
  /`apply_pinned` → `Overrides`; `encode`/`decode` persist dialog params
  under `KEY_LAYOUT` (global) — **M4 = per-view key `KEY_LAYOUT` + view
  key, the legacy global entry stays as fallback default; the dialog
  seeds from the stored view's `automaticLayout` FIRST**. Dialog
  `shell/actions.rs::layout_window` (engine combo, pin count + "Apply
  (ignore pins)" since M3); `run_layout(params, ignore_pins)`; pins =
  `LayoutOverlay.elements`; Shift-click pin toggle while the dialog is
  open (`shell/canvas.rs::canvas_click` + `pinned_ids`).
- dagre source audit lives at `/tmp/opencode/dagre-src/dagre-0.1.2/`
  (re-unpack from the cargo registry cache if gone).
- Write-back (M4): `ModelCommand::SetAutomaticLayout(view_key, from, to)`
  through `Viewer::run_model_edit` + gate `commands::model_editing_allowed`
  (plain `.json` only, never DSL/sidecar-preferred/stdin — F13 posture);
  undo restores the prior `Option<AutomaticLayout>`; the five STANDARD
  fields only; bless stays NO-OP; V5 = P6-style gate matrix test
  (`.json` allowed; `.dsl`/sidecar-preferred/stdin blocked) + write-back
  bytes are standard contract fields only.
- CLI (M6): `oxidizr view --layout-engine <layered|dagre|dot>` +
  completions; the CLI probably also wants a `dagre` passthrough feature
  (like viewer's) — decide there.

## If a next session opens

- Start wave **P7-M4** per the `build/PHASE_7.md` wave table (per-view
  params in the cache + legacy fallback, dialog seeds from stored view
  config, "Remember in view" = `ModelCommand::SetAutomaticLayout` through
  the model gate; V5 = `.json` allowed, `.dsl`/sidecar-preferred/stdin
  blocked + write-back bytes standard-contract-only). M3 evidence is in
  the plan row; do not re-derive it.
- Feature-matrix sanity before commit (no CI leg covers these):
  `cargo clippy -p oxidizr-layout --features dot-backend,dagre --all-targets`
  and `cargo clippy -p oxidizr-viewer --features gui,dagre,dot-backend --all-targets`.
- Gates: EVERY commit `cargo xtask ci gui fmt clippy test refs` — FULL
  form, every leg named; a bare `ci gui` runs ONE leg and still says
  "all steps green" (pitfall `xtask-ci-steps-replace-defaults.md`).
  Bless NO-OP every wave; bench budget changes recorded with numbers.
- Shipped-prose hygiene: milestone ids (`§M2`, `P7-M2`) in
  `crates/oxidizr-*/src` comments FAIL the `test`+`refs` legs — the
  self-gating xtask hygiene test scans them (note `refs-hygiene-prose-
  patterns.md`). Kinds/roles in prose, process ids only in `agents/`.
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
