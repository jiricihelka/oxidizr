# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-16 · HEAD = M6-a CLI-surface commit (tip of `master`) · gates green ·
nothing pushed.

- **Done:** P1-M6a CLI surface COMPLETE (`report.rs` miette `Issue`/`Renderer` with
  reference exception text + snippets; `dsl2json`/`json2json`/`validate` wired, JSON
  loader injected, `-` stdio, exit codes 0/1/2/3; D3 `--keep-unknown` =
  `oxidizr_json::roundtrip_keep_unknown` overlay in `keep.rs` + `Json::Raw`; codes
  OXZ-3001/4001/4002/5006 registered; 15 e2e tests — PROGRESS.md M6a log); P1-M5
  extends (9f174e6, c468854); P1-M4; infra through 2576a25. V-3 (env fallback ON)
  and V-8 (UTF-8 only, no flag) closed in PHASE_1 §P1-D12.
- **Next:** GOAL = finish Phase 1 → **M6-b formatter** (PHASE_1 §P1-D11): `fmt`
  over the lossless CST (`oxidizr-ast`, already a CLI dep), P1-D7 invariants
  (verbatim when already canonical, idempotent, stable), `--write`/`--check`,
  proptest round-trips; the `fmt` arm currently bails in `run()`. Then M6-c
  reconcile + validate polish (did-you-mean suggestions into `Issue::help`) +
  M6-d fuzz targets/CI nightlies. Then M7 (corpus < 100 ms check, README polish,
  v0.1.0).
- **Open questions:** none.
- **Don't re-derive (trust + spot-check against `ad34036`):** M4 findings in
  `agents/plan/PROGRESS.md` errata + `agents/plan/DSL-ERRATA.md`; M5 findings in
  PROGRESS M5 log + `testing/oracle-app-harness.md`; M6a findings in PROGRESS M6a
  log + `testing/cli-e2e-pitfalls.md` (piped stdio, random
  `structurizr.dsl.identifier` UUIDs, `revision` vs `lastModified*` known/legacy);
  JSON byte rules in `agents/plan/build/WRITE-CATALOG.md`; keep-mode pairing rules
  in `crates/oxidizr-json/src/keep.rs` module docs; env facts in `agents/notes/env/`;
  critcmp takes only in-root baseline names (`bench cmp` stages, see main.rs doc).
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
