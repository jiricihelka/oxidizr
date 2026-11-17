# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = T2 commit (tip of `main`, tag `v0.1.0` far down) ·
gates green · nothing pushed.

- **Done:** **PHASE 1 COMPLETE** (M0–M7, tag `v0.1.0` local). **PHASE 1T T0+T1+T2
   DONE**: T0 = `oxidizr-api` extraction; T1 = `oxidizr-e2e` harness + 14-entry
   seed + bless + coverage gate (`d8d19f5`). T2 (this commit) = valid corpus
   grown to **156 entries** (≥150 floor met): basics/hierarchy/rel 08–35, groups
   36–44, deployment 48–61, views 62–75, styles 76–85, perspectives/properties
   86–95, finders 96–105, extends incl. JSON base 106–113, include trees
   114–123, vars/consts 124–133, quoting/comments 134–143, archetypes 144–151,
   big composites 152–159. All blessed, audit-clean (every sidecar: three cmds
   exit-ok + fmt idempotent). V-T2: 163 entries in 36 ms single-threaded.
   Grammar pitfalls note: `pitfalls/dsl-corpus-grammar-gotchas.md` — READ before
   authoring fixtures.
- **Next:** **T3** per `build/PHASE_1T.md` — ≥120 invalid entries (coverage of
   the registry) + implement `cargo xtask e2e mutate` (currently a stub bail in
   `crates/xtask/src/main.rs`; deterministic seeded, V-T3 = same seed twice ⇒
   byte-identical output) + shrink coverage allow-list {1001, 5001, 3001, 3002,
   4002, 5005, 9001}. Then T4 (invariants behind `OXIDZR_E2E_FULL=1` + nightly
   `.github/workflows/e2e.yml` + V-T4 `#[ignore]` inventory), T5 (`e2e_` benches
   behind feature `e2e-bench` + `cargo xtask bench save 1T`), T6 close-out, no
   tag. Then Phase 2 (`build/PHASE_2.md`).
- **Open questions:** none. Flag for oracle check: engine rejects `url` inside
   `group` though vendored `testdata/dsl/group-url.dsl` exists — Phase-1 compat
   gap candidate, not yet in DEVIATIONS.md.
- **Don't re-derive (trust):** P1T-D1..D9 + T0/T1/T2 landed blocks
   (`build/PHASE_1T.md`); `pitfalls/dsl-corpus-grammar-gotchas.md` (trailing
   comments poison tokens, ASCII ids, `routing`/`iconPosition`/`jump` enums,
   deploymentNode 5th token = instances, hierarchical ids need dotted scopes,
   extends-env reuse via `!element <envId>`, styles-include needs `styles {}`
   wrapper at views level); `!script` never yields a clean single trigger;
   `!docs` → clean 5003 and `!impliedRelationships <fqcn>` → clean 5002 (T3
   invalid-fixture design input); NO_COLOR stderr still has ANSI (strip before
   grep); fuzz mingw-CC-strip (`env/fuzz-host-toolchain.md`); JSON byte rules
   `WRITE-CATALOG.md`.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
