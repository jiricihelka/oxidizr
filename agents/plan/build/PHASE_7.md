# PHASE_7.md — detailed plan: auto-layout quality (dagre) + configuration

**Status: PLANNED 2026-11-28** — scope ratified by the user via the questions
tool (2026-11-28, two rounds); implementation not started. Position: after
P6 (CLOSED 2026-11-28). The phase supersedes roadmap open question **O2**
(own engine, `PLAN.md` / PHASE_3 P3-D5) — see P7-D0. No tag at close.

User charter (verbatim, 2026-11-28): *"Plan phase P7 to improve the autolayout
algorithm and configuration. Propose solutions with their benefits and
drawback to the autolayouting problem using the questions tool."*

Ratification answers (questions tool, 2026-11-28 — the decision anchors):

- D1 quality strategy: **"You can add a dependency. It just has to be from
  crates.io and it has to not affect multiplatformness or compilation
  dificulty. It must be statically linked to the binary."**
- D2 boundaries: **compound layout** — perimeters become layout inputs.
- D3 pinning: **pin what I moved** — manual placements survive Apply.
- D4 config: **per-view params + write-through** into the stored view.
- D5/D9 dot: **"Do not include external dependencies"** — no PATH discovery,
  no dot UX push; dot-backend stays frozen but **joins the engine selection
  framework** (selectable when its feature + binary are present).
- D6 dependency: **dagre 0.1.2 first, with a multi-engine abstraction** —
  engines feature-enabled, GUI/CLI-selectable.
- D7 topology: **dagre default + Layered for pinned runs** (anchors need a
  coordinate pass we control).
- D8 pin semantics: **overlay positions = pins** (Shift-click unpin,
  "Apply (ignore pins)" escape hatch).

## Current state (measured 2026-11-28)

- `oxidizr-layout`: ZERO-dependency crate (P3-D1); `Layered` engine =
  longest-path layering + fixed barycenter sweeps + priority-method
  coordinates (explicit "no simplex at this scope"; E-3 rank tightening +
  median boundary + channel lanes already in); `Dot` subprocess behind
  `dot-backend` (off, `$OXIDZR_DOT_BIN` only); **determinism is a hard
  contract** (property test, stable sorts, no randomness); benches at 1k.
- `LayoutConfig` mirrors the five `automaticLayout` contract fields, but
  **`edge_separation` is ignored by Layered** (parity field only; dot maps
  it to `esep`).
- Boundaries are NOT layout inputs: viewer `layout.rs::graph_for` maps
  scene items+edges only → boundary members scatter, cross-boundary edges
  pierce boxes.
- Apply always relayouts EVERYTHING — overlay (manual) positions are lost.
- Dialog params are ONE GLOBAL set persisted via `layout::encode` under
  `KEY_LAYOUT` in the viewer cache — not per-view; the stored per-view
  `automaticLayout` seeds only the display path, not the dialog.
- dagre 0.1.2 (`crates.io/crates/dagre`, Actrium/supramark, Apache-2.0,
  edition 2024, ~14k SLoC): dagre.js port — compound subgraphs,
  network-simplex ranking, Brandes-Köpf coordinates, 4 rank directions,
  self-loops; ONLY mandatory dep `log` (serde/serde_json ride the
  default-features-off-able `json` feature). Determinism UNCLAIMED —
  P7-M0 must prove it (V1) before any default switch.

## Decisions

- **P7-D0** (supersedes O2/P3-D5): layout quality comes from a pure-Rust
  crates.io dependency, statically linked, cross-platform-safe.
  `dagre` is adopted behind a NON-default `dagre` cargo feature of
  `oxidizr-layout` — the published crate's default-feature tree stays
  zero-dependency (P3-D1 preserved where observable); the viewer enables
  the feature. `Layered` stays the reference/pinned engine; `dot-backend`
  unchanged otherwise (D5). Rationale: dagre is one crate + `log`;
  ELK ports were measured (17 crates / 56k SLoC / JSON boundary / EPL
  second-license) and rejected as heavier than the constraint allows.
- **P7-D1**: dagre is the DEFAULT engine (display auto-layout + dialog);
  Layered runs automatically when the pinned set is non-empty (P7-D3), and
  stays user-selectable. The `LayoutEngine` seam + `Engine::available()`
  become the engine framework: each engine is a feature + runtime
  availability, selectable in the dialog combo AND via
  `oxidizr view --layout-engine <layered|dagre|dot>` (cached choice).
- **P7-D2**: boundaries enter the graph as dagre COMPOUND PARENTS —
  `graph_for` grows membership (scene boundaries already carry members);
  boundary boxes stay derived chrome around laid-out members. Layered
  keeps today's flat behavior (its boundary-median pass unchanged).
- **P7-D3**: pin = element with an overlay position at Apply time. Pinned
  run = Layered with ANCHOR CLAMPS in the coordinate pass (pinned vertices
  hold exact positions; others relax around them — order and separations
  preserved, the existing priority-method machinery). Escape hatch
  "Apply (ignore pins)"; Shift-click toggles a pin; a pin badge marks
  pinned elements while the dialog is open.
- **P7-D4**: dialog params become PER-VIEW in the viewer cache
  (`KEY_LAYOUT` + view key; the legacy global entry remains as fallback
  default); the dialog seeds from the stored view's `automaticLayout`
  first; an opt-in **"Remember in view"** writes the five standard fields
  back into the stored view — a new `ModelCommand::SetAutomaticLayout`
  (invertible, one undoable step) through the P6 gate matrix: plain
  `.json` sources only, never DSL/sidecar-preferred/stdin (F13 posture).
- **P7-D5**: no dot UX investment. dot appears in the engine framework
  exactly as today (feature compiled + `$OXIDZR_DOT_BIN` set); no PATH
  discovery, no messaging.
- **P7-D6**: wire `edge_separation` into Layered (port-spread spacing —
  the dialog stops lying); dagre maps it natively (`edgesep`).
- **P7-D7**: determinism remains the hard contract for EVERY engine —
  dagre adoption is gated on the V1 proof; if dagre cannot be made
  deterministic the default switch (P7-D1) does NOT happen and reality
  goes back to the user (charter rule: plan-contradicting findings stop
  the wave).

## Waves (each: `ci gui fmt clippy test refs` FULL form + bless NO-OP green = commit; RESUME updated)

| Wave | Deliverable | Notes / verification |
|---|---|---|
| **P7-M0** | dep spike + engine framework: `dagre` behind the `dagre` feature; `Dagre` adapter implementing `LayoutEngine` (dagre `Graph` in, `LayoutResult` out — center→top-left, edge point polylines, multigraph + self-loops on); `Engine::available()`/dialog combo/CLI flag ride the framework; determinism property test; **quality-metric harness** (crossings + bends + edge length, E-3 metric, corpus table committed) | **DONE 2026-11-28. GATE V1 GREEN:** 116 corpus variants × 3 in-process repeats + 10 concurrent fresh processes, bit-exact (`tests/dagre_determinism.rs`, digest `32baf4be2345c6bf`, ~16 s); `Layered` covered by the same battery. V8 measured: default tree EMPTY; `dagre` feature adds 14 crates of which ONLY `dagre` is new to the viewer's headless tree (the `json`-feature serde pair already rides it) — zero windowing/GPU crates. MSRV/edition OK (dagre edition 2024, builds on our pin). Dialog combo live with `Engine::Dagre` (`Layered` stays default until M1). CLI flag lands at M6 with the rest of the CLI surface. See §M0 measured reality |
| **P7-M1** | dagre becomes the default engine (auto-display + dialog default); Layered selectable; corpus metric table recorded (dagre vs Layered per fixture) | V2: quality never below Layered on the corpus; scene-fixture snapshots re-blessed ONLY where metrics improve; bench cmp recorded |
| **P7-M2** | compound boundaries: `graph_for` emits boundary parents + membership for dagre runs; headless tests (members inside their boundary frame, cross-boundary edges attach sanely); Layered path untouched | V3 over every corpus view with boundaries |
| **P7-M3** | pinned tidy: overlay set = pins → Layered + anchor clamps (coordinate pass); exact-pin headless tests; pin badge + Shift-click unpin + "Apply (ignore pins)"; dialog explains the engine switch | V4: pinned elements byte-identical after Apply |
| **P7-M4** | config: per-view params in the cache (+ legacy fallback), dialog seeds from stored view config, **"Remember in view"** = `ModelCommand::SetAutomaticLayout` through the model gate (undo restores prior/absent) | V5: P6-style gate matrix (`.json` allowed; `.dsl`/sidecar-preferred/stdin blocked) + write-back bytes are standard contract fields only |
| **P7-M5** | `edgeSeparation` wired into Layered port spread; display fallback params re-checked against dagre defaults; dialog/labels honest again | metric-gated re-bless (V2) |
| **P7-M6** | CLI + docs: `oxidizr view --layout-engine` (+ completions), viewer README + shortcut/dialog polish, `CHANGELOG` review, manual smoke lists for M1–M4 written into the wave rows | quartet + gui |
| **P7-M7** | close: journals (PHASE_7/PROGRESS/RESUME), full V-list — full tier, doc-zero, `ci windows`, Category B oracle + export-verify (`TZ=UTC`), bless NO-OP, bench | V-list below; no tag |

## Verification

- V1 determinism: every enabled engine, corpus graphs, 10× byte-identical
  runs (property test, M0 gate before any default switch).
- V2 quality metric: committed per-fixture table (crossings/pierces/bends/
  length, `tests/layout_quality.rs`). **Amended 2026-11-28 at M0**: "dagre ≥
  Layered on every shape" is NOT attainable — six option profiles (network
  simplex / tight tree × greedy acyclicer × rank align) were swept and none
  dominates. The gate is now: the blessed table + the explicit
  `KNOWN_CROSSING_REGRESSIONS = ["c4-like", "grid-120"]` set (a NEW worse
  shape fails; silently fixing a listed one also fails — both force a
  deliberate re-read) + the TOTAL crossing count must improve. Snapshots
  re-blessed only where the metric improves, per-fixture in the row stamp
  (E-3 posture kept).
- V3 compound boundaries: headless scene tests over the corpus.
- V4 exact pinning: pinned coords unchanged; others moved.
- V5 write-through gate matrix + byte contract: `e2e bless` NO-OP every
  wave; write-back touches only the five `automaticLayout` fields.
- V6 per-commit: `cargo xtask ci gui fmt clippy test refs` FULL form (the
  bare `ci gui` form is NEVER a gate — pitfall note
  `xtask-ci-steps-replace-defaults.md`) + bless porcelain empty.
- V7 close: full tier, doc-zero (`RUSTDOCFLAGS="-D warnings" cargo doc
  --workspace --no-deps`), `cargo xtask ci windows` (dagre must cross),
  Category B (`$OXIDZR_STRUCTURIZR_BIN`, `TZ=UTC`), bench budget updated
  with evidence (dagre is allowed to be slower — recorded, not hidden).
- V8 tree promise: `oxidizr-layout` default features = zero deps (measured
  in CI-visible wave row); viewer headless tree = no windowing crates
  (everything dagre brings is pure computation). The `dagre` feature cannot
  ship `default-features = false` — dagre 0.1.2's ungated `graph/json.rs`
  makes the `json` feature (serde/serde_json) mandatory; the pair already
  rides the viewer tree, `dagre` itself is the only genuinely new crate
  (`agents/notes/env/dagre-crate-json-feature.md`).

## M0 measured reality (2026-11-28)

Three findings the plan could not have carried, each resolved inside M0's
scope (none contradicts P7-D0…D7, so no STOP per P7-D7):

1. **dagre 0.1.2 does not build `default-features = false`.** `graph/json.rs`
   is an ungated `pub mod` that unconditionally uses `serde` — turning the
   `json` feature off is a compile error, not a dependency win. `features =
   ["json"]` is forced on; serde/serde_json already ride the viewer tree, so
   the V8 promise holds in substance (the only new crate is `dagre` itself).
   Upstream defect + measured trees: `agents/notes/env/dagre-crate-json-feature.md`.
2. **V2 redesigned** (see the amended bullet): no option profile dominates,
   the committed table + named-regression-set + aggregate gate replaced the
   per-shape floor. Measured LR/contract separations: TOTAL crossings
   1503→1005, pierces 304→191, bends 2974→2385; regressions only on
   `c4-like` (+1 crossing, against fewer bends/shorter routes) and the
   random `grid-120` torture shape (Layered's channel lanes win; its grid
   numbers are additionally flattered by the `edge_separation` gap that M5
   closes for both engines).
3. **`oxidizr-layout --features dot-backend` was broken since P5R-R5b**
   (`e76ba12` demoted `PX_PER_INCH`/`COMPONENT_GAP_PX` to `pub(crate)` but
   left them on the `pub use` line → E0364; no CI leg enables the feature,
   so it stayed latent). Fixed by dropping the two private names from the
   re-export — a frozen backend cannot "join the engine selection" if it
   does not compile. Lesson noted: the dagre feature now has a local check
   (`cargo check -p oxidizr-layout --features dot-backend,dagre`) but no CI
   leg either — closing that is M7's call.

Also measured: the corpus metric harness initially over-counted crossings on
**both** engines because `f64::signum(0.0) == 1.0` treats a point exactly on a
segment's line as one side; the metric hand-rolls the sign now and carries a
self-test (touching/collinear routes must not count).

## Risks

- **R-1** dagre nondeterminism (HashMap iteration, float order): M0 V1 gate
  catches it BEFORE adoption; fallback = upstream-friendly fix behind our
  adapter, or Layered stays default (P7-D7).
- **R-2** dagre maturity (0.1.x, 43% docs): adapter isolates the API;
  Layered fallback keeps every rollback local.
- **R-3** compound edge cases (empty boundary, single member, nested):
  fixture battery in M2; degenerate clusters collapse to plain members.
- **R-4** pinned runs choose the weaker engine → quality gap surprise; the
  dialog states which engine ran (label stays visible after Apply).
- **R-5** bench regression at 1k nodes: measured at M0/M1, budgets updated
  with recorded numbers, not silently.
- **R-6** cache schema growth: legacy `KEY_LAYOUT` read-fallback keeps old
  caches valid; no migration job.
