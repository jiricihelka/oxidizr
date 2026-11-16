# bench-baseline — committed criterion baselines (agents/plan/TESTING.md §Benchmarks)

One directory per milestone (`M0`, `M3`, `M4`, `M7`). Contents: criterion's `new/`
output per benchmark (`estimates.json`, `sample.json`, `tukey.json`, `benchmark.json`).
Machine-relative — compare only against the same hardware (use `critcmp` against
`target/criterion`). Baselines recorded with the pinned toolchain of the milestone.

M0 context: nightly-2026-11-13 (rustc 1.102.0-nightly 8eb778d04), linux x86_64 dev box,
`cargo bench --workspace`, 2026-11-14. Only benchmark: `m0_scaffold_scan`
(plumbing smoke: lines+chars scan of a synthetic DSL string; real targets land M4/M6/M3).

M7 context: same toolchain and box, `cargo xtask bench`, 2026-11-16 — FIRST REAL
BASELINE (mandated targets of TESTING.md §Benchmarks all land here):
`m7_dsl_parse_corpus` (whole `testdata/dsl` UTF-8 corpus, ~11.5 ms median — the
< 100 ms perf-sanity check passes with ~9x margin), `m7_dsl_parse_test.dsl` /
`m7_dsl_parse_big-bank-plc.dsl` (biggest fixtures, worst-case proxies),
`m7_dsl_fmt_corpus`, `m7_json_read_corpus` / `m7_json_write_corpus`
(goldens + read-corpus + validation + probes), `m7_reconcile_matched_big_bank` /
`m7_reconcile_cross_corpus`. `m0_scaffold_scan` kept as the cross-milestone
anchor.
