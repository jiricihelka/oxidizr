# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-17 · HEAD = T0 close-out docs commit (tip of `main`, tag
`v0.1.0` several commits down) · gates green · nothing pushed.

- **Done:** **PHASE 1 COMPLETE** (M0–M7, tag `v0.1.0` local). **PHASE 1T T0
  DONE**: T0-a `EnvLookup` seam `5eedf99`; T0-b `oxidizr-api` crate +
  `Issue`/`Snippet` move `52a83c1`; T0-c `suggest` move `075cb90`; T0-d
  (this commit): command bodies + `reconcile` extracted into
  `oxidizr-api::pipeline` (`Request → execute() → Outcome{exit, log, bytes}`);
  CLI `run()` = argv→Request / Outcome→replay→i32 shim, zero logic. Engine
  seam upgrades along the way: `EnvLookup` is an Arc newtype (captured
  hermetic map), `ParseOptions` Clone-not-Copy. Byte-proof held: all 22 CLI
  byte-e2e + goldens pass UNMODIFIED (reconcile/suggest unit tests relocated
  with their modules). Stdin is lazy (`Input::Stdin` + injected
  `StdinReader`) — `fmt --write -` exits 2 without touching the pipe (verified
  against a live pipe). Manual exit-code battery 0/1/2/3 all match.
- **Next:** **T1** per `build/PHASE_1T.md` (T0 DONE — refinements recorded in
  its D1 "T0 landed" block): `oxidizr-e2e` crate + manifest sidecar reader/writer +
  `cargo xtask e2e bless` + ~10 seed entries + coverage skeleton + subprocess
  smoke (`CARGO_BIN_EXE_oxidizr`); measure V-T2 quick-tier budget. Then T2
  (≥150 valid), T3 (≥120 invalid + `xtask e2e mutate`), T4 (invariants +
  nightly `e2e.yml`), T5 (`e2e_` benches behind feature `e2e-bench` +
  `bench save 1T`), T6 close-out, no tag.
- **Open questions:** none.
- **Don't re-derive (trust):** P1T-D1..D9 decisions (`build/PHASE_1T.md`);
  the shim invariants above (esp. WHY stdin is lazy + bytes-before-log replay
  order + log suppression on stdout-write failure — all contract-pinned by
  the byte-e2e tests); `ParseOptions` now has 4 fields + Clone (child opts
  clones `env_lookup`); fuzz mingw-CC-strip; `testing/cli-e2e-pitfalls.md`;
  `env/fuzz-host-toolchain.md`; JSON byte rules `WRITE-CATALOG.md`.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
