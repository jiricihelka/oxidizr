# RESUME — session checkpoint (updated 2026-11-28, **PHASE 7 CLOSED — waves M0…M7 all DONE and committed (`f14b7b0` plan, `387f452` M0 … close commit; no tag by design). Previous arcs: P6 CLOSED 2026-11-28 (`9c4a4bf`…`41f9ddf`, no tag); P5R CLOSED 2026-11-26, tag `v0.5.0` LOCAL. Never pushed — push/publish stay the user's call. NO open work items on the agent side.**)

## What P7 delivered (evidence in `build/PHASE_7.md` wave rows — do not re-derive)

- **dagre 0.1.2 is the DEFAULT layout engine** — pure Rust, crates.io,
  statically linked; NON-default `dagre` feature on `oxidizr-layout`
  (zero-dep library promise kept), viewer `default = ["dagre"]`,
  hand-written `Engine::default()` (`Layered` in `--no-default-features`
  builds). V1 determinism PROVEN before the flip (116 variants × 3
  in-process + 10 concurrent processes, bit-identical).
- **Compound boundary layout** (dagre parents; members stay in frames).
- **Pins**: overlay entries ARE pins, exact hold via Layered anchor
  clamps, Shift-click toggle while the dialog is open, "Apply (ignore
  pins)".
- **Per-view layout config** (`KEY_LAYOUT/<view>` cache + legacy global
  fallback) + **"Remember in view"** (five standard `automaticLayout`
  fields via `ModelCommand::SetAutomaticLayout`, one undoable step,
  plain `.json` only).
- **`edgeSeparation` honoured by BOTH engines** — Layered fans sit
  centred at the exact spacing when the side fits, equal slots
  otherwise, N=1 byte-unchanged.
- **`--layout-engine <layered|dagre|dot>`** on `oxidizr view` AND the
  viewer binary through the ONE `shell::run(file, theme, engine)` seam;
  unavailable request = default kept + dialog note; completions render
  it. dot stays frozen (feature + `$OXIDZR_DOT_BIN`, no new external
  dependency investment — P7-D9).
- Quality (blessed table, `KNOWN_CROSSING_REGRESSIONS = ["c4-like",
  "grid-120"]` gate): TOTAL crossings Layered 1490 / dagre-as-default
  1005 vs 1503 pre-P7; pierces 304→293/191; bends 3006→3002/498.
- Close leg results: full tier ok; doc-zero green after ONE real fix
  (public `apply_pinned` doc must not link the crate-private seam);
  windows/Wine green (dagre RUNS under Wine — V7); oracle 4+4+99+6 and
  export-verify 31 ×2 formats (TZ=UTC); bless NO-OP every wave; bench:
  dagre 1k unchanged, Layered +2.7–7.5% recorded (M5 µs-scale cost,
  R-5 gap stands).

## If a next session opens

- **Nothing is in flight.** P7 closed without a tag (by design, like
  P6); tags `v0.1.0…v0.5.0` LOCAL; never pushed.
- The user-side leftovers are NOT agent tasks: the manual GUI smoke
  lists written into the PHASE_7 wave rows (M1–M5) want an interactive
  pass; push/publish is the user's act.
- Next scope = whatever the user sets (roadmap `agents/plan/PLAN.md`;
  O2 was superseded by P7-D0). A new phase gets its `build/PHASE_N.md`
  planned first — charter says planning = questions + recorded
  decisions.
- Gates unchanged: EVERY commit `cargo xtask ci gui fmt clippy test
  refs` — FULL form, every leg named (pitfall
  `xtask-ci-steps-replace-defaults.md`); bless NO-OP = empty
  `git status --porcelain testdata/`; feature matrices
  (`oxidizr-layout --features dot-backend,dagre`,
  `oxidizr-viewer --features gui,dagre,dot-backend`) are clippy-clean
  but have NO CI leg — check before touching those crates.
- Quality harness re-bless: `UPDATE_EXPECT=1 cargo test -p
  oxidizr-layout --all-features --test layout_quality`; the V2 gates
  (dagre columns byte-unchanged unless the change is IN dagre code,
  verdict set unchanged, TOTAL improves) are enforced by the test
  itself only for crossings — check the rest manually.

## Pitfalls earned this phase (all also noted in wave rows/notes)

- Clap: a doc comment alone does NOT stop snake_case flattening
  (`layout_engine` → `--layoutengine`); pass explicit
  `long = "layout-engine"` (precedent `--ignore-layout`).
- `background` for the shell tool is a lowercase JSON boolean.
- Multi-line commit messages: write to a file, `git commit -F <file>`.
- Shipped-prose hygiene: no milestone ids / provenance narration in
  `crates/oxidizr-*/src` (the `refs` leg enforces).
- `cargo xtask ci <steps>` REPLACES the quartet — name every leg.
- rustdoc: a public item's doc may not intra-doc-link a
  `pub(crate)` item (doc-zero fails it) — plain code span instead.
- f64: `signum(0.0) == 1.0` (crossing tests must hand-roll the side
  test — note `f64-signum-zero-is-one.md`).
- dagre 0.1.2 cannot build `default-features = false` (ungated serde in
  `graph/json.rs`) — note `dagre-crate-json-feature.md`.

## Workflow (measured, `agents/notes/process/parallelism.md`)

- NO subagents by default (measured memory thrash; hard cap 3).
  Parallelize harness cycles via background shell jobs; two cargo jobs
  queue on the target-dir lock — chain in one background command; never
  poll background shells.
- Gate-log reading: `grep -c` exits 1 on zero matches — printed `0` is
  GREEN; verify `ci: all steps green` in the log body.
- Category B env: `$OXIDZR_STRUCTURIZR_BIN=/nix/store/
  5yil7chk3cjg3qmsabq1vzdxv0v4bwm0-structurizr-2026.10.26/bin/structurizr`
  (dev shell exports it), legs `TZ=UTC`, inside `nix develop
  --command …`, background only.
- Windows leg: `nix develop --command cargo xtask ci windows` (Wine,
  slow — full suite).
- `dist plan` = `nix develop --command dist plan` (not an xtask
  subcommand).
