# RESUME — session checkpoint (updated 2026-11-18, P3 planning)

## State

- **Phase 2 shipped**: tag `v0.2.0` (local, on `b735732`), gates + full oracle verify
  green, nothing pushed (never push). P2 retrospective + P3 strategy proposals were
  delivered in chat; adoption decision pending from the user (S1–S8).
- **Now: P3 PLANNING** — no implementation until the plan is finalized.
- Gate: `cargo xtask ci`; oracle 2026.10.26 (slow — weekly); mmdc absent; NO DNS.

## In flight

- `agents/plan/build/PHASE_3.md` now carries the ratified **P3-D0 parity contract**
  (feature parity, NOT visual parity — GUI looks like us, canvas renders from theme
  data) + **P3-F feature matrix** verified against the pinned monorepo
  (`structurizr-application` diagrams fragments + `structurizr-diagram.js`).
- **Waiting on the user's answers to Q1–Q4** (Q1 theme-driven canvas = ✅ data;
  Q2 PNG/SVG export in P3?; Q3 extras bundle key/tooltips/tag-filter/align/canvas,
  minimap+search→P4; Q4 custom/image view rendering depth). On answers: resolve the
  ❓ rows + the "Open sign-offs" paragraph in PHASE_3.md, record outcomes as
  P3-D items, commit plan: …
- Reference GUI facts learned (verified): 9 view kinds (`ViewSet.java:735–860`);
  editor = align/distribute/centre, canvas size ±/auto, auto-layout dialog
  (direction, generate-vertices, separations, bring-back, remove-vertices);
  viewer = key `[i]`, tooltips `[t]`, tag filter, navigator, animation (2 s),
  PNG/SVG export, embed, perspectives/health/explore (service-side → N/A).
  Layout storage: x/y per elementView + vertices/position(0–100)/routing/jump per
  relationshipView, merged on push via `copyLayoutInformationFrom` /
  `DefaultLayoutMergeStrategy` (elements by canonical name→name→description,
  relationships by id) — this is the P3 geometry contract (P3-S1 from the retro).

## Workflow reminders

- Never push; commit every green sub-piece; rewrite this file each commit; gates
  before commit; JSON goldens only via oracle; reference monorepo
  `/tmp/nix-shell.uni100/opencode/structurizr` (`ad34036`); witness sweeps under
  `/tmp/nix-shell.uni100/opencode/m2d/sweep` (30 fixtures × ref-{plantuml,c4,mermaid}).
- Reference source tree is FLAT (`structurizr-<module>/src/main/java/...`).
