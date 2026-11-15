# NIGHTLY.md — nightly toolchain policy & feature register

Policy (agents/plan/WORKFLOW.md §Dev environment / §Code quality): the toolchain is pinned in
`rust-toolchain.toml`; only reasonably stable nightly features may be used (long
incubation or an active stabilization path — no specialization-class megafeatures).
Every usage gets a row below with a one-line rationale, and this file is re-checked
for stabilization at each milestone.

## Current pin

| Item | Value |
|---|---|
| channel | `nightly-2026-11-13` (= rustc 1.102.0-nightly, commit `8eb778d04` 2026-11-12) |
| flake parity | `flake.lock` rust-overlay rev `84edd48f` (2026-11-13) → same nightly |
| set at | P1-M0 (2026-11-14) |
| bumped by | milestone process (`cargo xtask` notes + CI keep 3-OS green) |

Why nightly at all: `cargo-fuzz` targets (P1-M6) and future stabilized-path features;
library code itself is plain-stable-compatible (`rust-version = 1.85`).

## Feature register

| Feature | Crate(s) | Rationale | Stabilization status | Added | Removed |
|---|---|---|---|---|---|
| _(none yet)_ | | | | | |

Category A tests, benches and the shipped binary must build on the pinned nightly;
downstream stable builds are a non-goal while pinned (recorded deviation for
source-builders, per policy).
