# cargo bench arg forwarding

`cargo bench --workspace -- --report-dir DIR` forwards the args to EVERY bench
harness, including the libtest-generated ones (`--lib` per crate) — libtest dies
with `error: Unrecognized option: 'report-dir'` and exit 1. Observed in CI on all
3 OSes (2026-11-15, commit 487c5f7 fixed it).

Rule: don't pass criterion flags through `cargo bench --workspace --` in a
mixed-workspace repo. Criterion's default report root is `target/criterion`
anyway (CI uploads that path). Same trap class: `cargo test -- --<bench-flag>`.

Last useful: never
