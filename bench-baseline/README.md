# bench-baseline — committed criterion baselines (agents/plan/TESTING.md §Benchmarks)

One directory per milestone (`M0`, `M4`, `M7`) or baseline-carrying interlude
(`1T`, `2T`, `2R`). Contents: criterion's `new/`
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

Interlude baselines (same toolchain and box, `cargo xtask bench`): `1T` (2026-11-17,
`ea647d2` — whole-corpus e2e benches land), `2T` (2026-11-18, `7296929` — corpus export
benches plantuml/c4/mermaid, the current 23-group suite). `2R` context: 2026-11-19,
post-refactor tree (P2R PR6 close, `agents/plan/build/PHASE_2R.md`) — same 23-group
suite as `2T`; every group faster-or-equal (`m7_dsl_parse_corpus` 13.7→7.1 ms,
`m7_dsl_parse_big-bank-plc` 3.0→1.02 ms, `m7_reconcile_cross_corpus` 441→188 µs,
`e2e_dsl2json_all` 14.6→10.1 ms).
