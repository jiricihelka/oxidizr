# local CI mirrors (`cargo xtask ci|bench|cov`)

Step argv is literally the `run:` lines of `.github/workflows/ci.yml` — dev and CI
can't drift (commit efba278).

- `cargo xtask ci [STEP…]` — default `fmt clippy test refs` (the commit gate); no
  fail-fast, prints per-step banner + final summary of failed steps. `refs` is a
  local-only extra gate (doc-reference integrity, `crates/xtask/src/refs.rs`).
  `lint` (actionlint) is opt-in here and a CI job; `ci windows` ends with
  `wineserver -k` so no ~100 wine processes survive the leg. Bench baselines:
  `cargo xtask bench save <LABEL>` / `bench cmp <LABEL>` (critcmp, dev shell).
- `cargo xtask ci windows` — the windows-gnu+Wine leg; bails with a pointer to
  `nix develop` when `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER` is unset
  (env check = honest precondition). macOS stays CI-only.
- `cargo xtask bench` — bare `cargo bench --workspace` (NO `-- --flag` forwarding,
  see pitfalls/cargo-bench-args.md); reports under `target/criterion`.
- `cargo xtask cov` — `cargo llvm-cov --workspace --summary-only`, report only,
  never a gate; friendly probe when cargo-llvm-cov is missing.
- `cargo xtask oracle gen|verify` — Category B (see env/reference-oracle.md).

CI caching layers (for context when timings look odd): Swatinem/rust-cache for
target/+$CARGO_HOME, local action `.github/actions/toolchain-cache` for the pinned
toolchain dir, actions/cache for the structurizr war. First run after any pin or
lockfile change is a cold (seeding) run.

Last useful: never
