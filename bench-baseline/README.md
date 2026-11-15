# bench-baseline — committed criterion baselines (agents/plan/TESTING.md §Benchmarks)

One directory per milestone (`M0`, `M3`, `M4`, `M7`). Contents: criterion's `new/`
output per benchmark (`estimates.json`, `sample.json`, `tukey.json`, `benchmark.json`).
Machine-relative — compare only against the same hardware (use `critcmp` against
`target/criterion`). Baselines recorded with the pinned toolchain of the milestone.

M0 context: nightly-2026-11-13 (rustc 1.102.0-nightly 8eb778d04), linux x86_64 dev box,
`cargo bench --workspace`, 2026-11-14. Only benchmark: `m0_scaffold_scan`
(plumbing smoke: lines+chars scan of a synthetic DSL string; real targets land M4/M6/M3).
