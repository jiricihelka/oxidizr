# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-16 · HEAD = M6-d fuzz commit (tip of `master`) ·
gates green · nothing pushed.

- **Done:** P1-M6 **COMPLETE** (a: CLI surface; b: fmt; c: reconcile 7351db1 +
  suggestions 67af536; d: this commit — `fuzz/` cargo-fuzz workspace excluded
  from the main workspace, targets dsl_parse/dsl_fmt/json_read, `cargo xtask
  fuzz [SECONDS]` seeds from testdata into gitignored `fuzz/corpus/`, time-boxed
  runs; nightly `.github/workflows/fuzz.yml`; mingw CC/CXX leak trap →
  `env/fuzz-host-toolchain.md`). P1-M5 extends; P1-M4; infra through 2576a25.
- **Next:** **M7** (last milestone of Phase 1): ① perf sanity — release build,
  parse whole testdata corpus < 100 ms *informational* (check `crates/*/benches`
  first — criterion bench job exists; measure parse-only, not process startup);
  ② README quickstart (repo has NO README yet — root README.md, `--help` already
  done M6-a); ③ close-out entries (PHASE_1 §P1-D11 M7 DONE + PROGRESS board row,
  Phase-1 summary); ④ tag `v0.1.0` locally (NEVER push). Then Phase 1 is done —
  report to user before starting P2.
- **Open questions:** none.
- **Don't re-derive (trust + spot-check against `ad34036`):** fuzz design + CC
  strip (PROGRESS M6d log + env note); reconcile/suggestions contracts (M6c
  entries there + module headers); fmt line-ends `pitfalls/fmt-line-end-guards.md`;
  M4 findings PROGRESS errata + `agents/plan/DSL-ERRATA.md`; M5
  `testing/oracle-app-harness.md`; M6a `testing/cli-e2e-pitfalls.md` (EPIPE #6);
  JSON byte rules `agents/plan/build/WRITE-CATALOG.md`; env `agents/notes/env/`.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
