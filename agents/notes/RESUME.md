# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-16 · HEAD = **M7 commit + tag `v0.1.0` (Phase 1 COMPLETE)**,
tip of `master` · gates green · nothing pushed (tag stays LOCAL until the user
says otherwise).

- **Done:** **PHASE 1 COMPLETE** — M0 infra, M1 lexer/CST, M2 core+validation,
  M3a write catalog, M3b JSON writer/reader, M4 DSL engine, M5 extends,
  M6-a CLI surface, M6-b fmt, M6-c reconcile+suggestions, M6-d fuzz nightlies,
  M7 (this commit): mandated benches (`m7_dsl_parse_corpus` ≈ 11.5 ms ≪ 100 ms
  sanity bar; fmt/read/write/reconcile targets), first real baseline
  `bench-baseline/M7/`, root `README.md` quickstart, close-out entries in
  PHASE_1 §P1-D11 + PROGRESS board/log. Tag `v0.1.0` local.
- **Next:** Phase 2 planning is the USER's call — do NOT start P2 unprompted
  (AGENTS.md: report Phase 1 complete first). If asked: read
  `agents/plan/README.md` reading order, then the P2 exporter plan (create
  `agents/plan/build/PHASE_2.md` per the charter — questions welcome in
  planning). Pushing `master` + the tag is also user-only.
- **Open questions:** none.
- **Don't re-derive (trust + spot-check against `ad34036`):** bench corpus
  conventions + timings (PROGRESS P1-M7 log); fuzz design + mingw CC strip
  (P1-M6d log + `env/fuzz-host-toolchain.md`); reconcile/suggestions contracts
  (P1-M6c log + module headers); fmt line-ends
  `pitfalls/fmt-line-end-guards.md`; M4 findings PROGRESS errata +
  `agents/plan/DSL-ERRATA.md`; M5 `testing/oracle-app-harness.md`; M6a
  `testing/cli-e2e-pitfalls.md` (EPIPE #6); JSON byte rules
  `agents/plan/build/WRITE-CATALOG.md`; env `agents/notes/env/`.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
