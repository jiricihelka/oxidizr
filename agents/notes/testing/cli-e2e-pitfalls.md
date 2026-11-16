# CLI e2e test pitfalls (M6-a, 2026-11-16)

Hard-won facts for `crates/oxidizr-cli/tests/cli.rs`-style subprocess tests:

1. `Child::wait_with_output()` captures nothing unless you set
   `Stdio::piped()` for **stdout/stderr too** (stdin alone is not enough) —
   otherwise assertions see empty `stdout`/`stderr` while the child's real
   output leaks into the test harness's own streams.
2. Every DSL parse stamps `structurizr.dsl.identifier` with a **fresh random
   UUID** per element (reference behaviour). Any stdin-vs-file (or
   parse-vs-reparse) byte comparison must filter lines containing that
   property first.
3. `lastModifiedDate/lastModifiedUser/lastModifiedAgent` are **known**
   workspace fields — they round-trip in parity mode. The legacy key the
   reference reader drops (and `--keep-unknown` preserves) is `revision`
   (also `enterprise`, element-level `location`, …).
4. stdin has no file context (`getFile() == null`), so relative
   `workspace extends`/`!include` bases skip and `!include <dir>` resolution
   differs — never use fixtures with relative references in
   stdin-vs-file equivalence tests; use a self-contained source instead.
5. `NO_COLOR=1` in the test env + `GraphicalTheme::ascii()` keeps miette
   stderr deterministic (no ANSI in `contains` checks).
6. Piping stdin to a child that **exits before reading it** (usage errors,
   exit 2) races: `write_all` may hit **BrokenPipe** (EPIPE). The shared
   `run()` helper tolerates exactly that error kind — do not "fix" it back
   to a bare `.unwrap()` (it is a timing flake, green runs hide it).

Last useful: 2026-11-16
