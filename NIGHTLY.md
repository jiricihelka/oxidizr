# NIGHTLY.md — pinned nightly toolchain & feature register

Policy (AGENTS.md §Code quality guidelines): the toolchain is pinned in
`rust-toolchain.toml` (date set at M0). Only reasonably stable nightly features —
active stabilization path or long incubation — may be used, and **every** usage is
recorded below with rationale. Re-check this table at each milestone; when a feature
stabilizes, remove the `#![feature]` gate and mark the row done.

| Feature | Used in | Rationale | Stabilization | Status |
| --- | --- | --- | --- | --- |
| _(none yet — add rows at M0+)_ | | | | |

Category A tests, benches and the shipped binary must build on the pinned nightly;
downstream stable builds are a non-goal while pinned (recorded deviation for
source-builders, noted here per policy).
