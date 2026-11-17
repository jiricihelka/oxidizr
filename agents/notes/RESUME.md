# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = T1 commit (tip of `main`, tag `v0.1.0` far down) ·
gates green · nothing pushed.

- **Done:** **PHASE 1 COMPLETE** (M0–M7, tag `v0.1.0` local). **PHASE 1T T0+T1
  DONE**: T0 = `oxidizr-api` extraction (`5eedf99`…`59c4296`, byte-proof held).
  T1 (this commit) = `oxidizr-e2e` harness crate (corpus discover / manifest
  sidecar / runner observe+check / bless), `cargo xtask e2e bless [FILTER]`,
  14 seed entries (7 valid incl. JSON-only + deployment + extends-json;
  7 invalid covering 2001/2002/2003/4001/5002+5003/5006), quick fan-out
  (`thread::scope`), coverage gate (registry × allow-list, stale entries
  fail), subprocess smoke. V-T2 closed: 10 ms / 14 entries (~0.7 ms/entry,
  ×25 ≈ 0.3 s). Bless idempotent (repeat runs: all unchanged).
- **Next:** **T2** per `build/PHASE_1T.md` — grow valid corpus to ≥150 across
  the D2 axes (multi-file composites, dynamic/filtered views, constants,
  `${VAR}`, `!ref`s, quoting edges, hierarchical ids, all 8 view kinds);
  bless + review every sidecar; re-measure V-T2. Then T3 (≥120 invalid +
  `xtask e2e mutate` — currently a stub bail — + shrink allow-list: 1001,
  5001, 3001, 3002, 4002, 5005, 9001), T4 (invariants behind
  `OXIDZR_E2E_FULL=1` + nightly `e2e.yml`), T5 (`e2e_` benches behind feature
  `e2e-bench` + `bench save 1T`), T6 close-out, no tag. Then Phase 2
  (`build/PHASE_2.md`).
- **Open questions:** none.
- **Don't re-derive (trust):** P1T-D1..D9 + the T0/T1 landed-refinement blocks
  (`build/PHASE_1T.md`); entry inputs = `workspace.dsl` and/or
  `workspace.json` (json = explicit json2json feed, validate target when no
  dsl); malformed `extends <base.json>` = 2002 + recovery cascade (NOT 4001 —
  the dsl crate wraps loader errors as statement errors); `CARGO_BIN_EXE_*`
  is NOT exported for dev-dep bins (smoke walks up from current_exe;
  `--workspace` test builds the bin); DSL grammar edges: element/relationship
  `properties` must be BLOCK form, `workspace extends <f>` takes no
  name/description, `!script` cascades (never a clean single trigger);
  NO_COLOR stderr still has ANSI (strip before grep); fuzz mingw-CC-strip;
  `env/fuzz-host-toolchain.md`; JSON byte rules `WRITE-CATALOG.md`.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
