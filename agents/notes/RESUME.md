# RESUME — session checkpoint (updated 2026-11-18, P3 planning)

## State

- **Phase 2 shipped**: tag `v0.2.0` (local, on `b735732`), gates + full oracle verify
  green, nothing pushed (never push).
- **Phase 3 plan FINAL** (`agents/plan/build/PHASE_3.md`): P3-D0 parity contract
  (feature parity, NOT visual; editor rule = JSON values only, never .dsl) + P3-F
  matrix signed off by user 2026-11-18: Q1 theme data drives canvas; Q2 PNG/SVG
  export IN (M4); Q3 ALL viewing/navigation features IN incl. minimap + quick-nav;
  Q4 custom/image views = placeholder boxes only. **Implementation may proceed
  unsupervised from P3-M0** per milestones D9; open empirical items are P3-V1..V8
  (V7 filter persistence, V8 export anatomy are new).
- Gate: `cargo xtask ci`; oracle 2026.10.26 (slow — weekly); mmdc absent; NO DNS.
- P2 retro strategy proposals S1–S8 were delivered in chat; the plan already encodes
  S1 (geometry JSON = the parity contract, P3-D8) — adopt the rest into
  `agents/notes/process/` only if the user greenlights.

## Reference GUI facts (verified 2026-11-18, pinned monorepo)

- 9 view kinds (`ViewSet.java:735–860`); editor = align/distribute/centre, canvas
  size ±/auto, auto-layout dialog (direction, generate-vertices, separations,
  bring-back, remove-vertices); viewer = key `[i]`, tooltips `[t]`, tag filter,
  navigator, animation (2 s), PNG/SVG export, embed; perspectives/health/explore =
  service-side (N/A). Layout storage: x/y per elementView +
  vertices/position(0–100)/routing/jump per relationshipView, merged on push via
  `copyLayoutInformationFrom` / `DefaultLayoutMergeStrategy` (elements by canonical
  name→name→description, relationships by id).

## Workflow reminders

- Never push; commit every green sub-piece; rewrite this file each commit; gates
  before commit; JSON goldens only via oracle; reference monorepo
  `/tmp/nix-shell.uni100/opencode/structurizr` (`ad34036`); witness sweeps under
  `/tmp/nix-shell.uni100/opencode/m2d/sweep` (30 fixtures × ref-{plantuml,c4,mermaid}).
- Reference source tree is FLAT (`structurizr-<module>/src/main/java/...`).
