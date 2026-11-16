# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-16 · HEAD = M6-b formatter commit (tip of `master`) · gates green ·
nothing pushed.

- **Done:** P1-M6b `fmt` COMPLETE (`oxidizr_dsl::fmt` printer over the lossless CST,
  P1-D7 invariants + line-end guards — `agents/notes/pitfalls/fmt-line-end-guards.md`;
  gates `tests/m6_fmt.rs`: corpus idempotence + invariant-1 workspace equality
  (`structurizr.dsl` line dropped, UUIDs redacted) + proptest 20k cases, regressions
  committed; two latent M1 `lex_comment` tiling bugs fixed + regression; CLI `fmt`
  wired stdout/`--write`/`--check`, e2e 15→20; `oxidizr-ast` pruned from CLI deps);
  P1-M6a CLI surface; P1-M5 extends (9f174e6, c468854); P1-M4; infra through 2576a25.
  V-3/V-8 closed (PHASE_1 §P1-D12).
- **Next:** GOAL = finish Phase 1 → **M6-c** (PHASE_1 §P1-D11): `reconcile`
  (identifier-keyed layout remap — P1-D8 contract: `--dsl X --layout Y [-o]`,
  `structurizr.dsl.identifier` exact-match first, canonical-name fallback,
  ambiguous = OXZ-3xxx errors, never guess; human report of remapped/dropped/
  ambiguous) + validate polish (did-you-mean suggestions into `Issue::help`);
  the `reconcile` arm still bails in `run()`. Then **M6-d** fuzz targets + CI
  nightlies, then **M7** (release-build corpus < 100 ms check, README quickstart,
  v0.1.0 tag).
- **Open questions:** none.
- **Don't re-derive (trust + spot-check against `ad34036`):** fmt line-end/`split(
  "\\r?\\n")`/joiner-strip contract + its guards in `pitfalls/fmt-line-end-guards.md`
  and the `fmt.rs` module header; M4 findings in PROGRESS errata + `agents/plan/
  DSL-ERRATA.md`; M5 findings in PROGRESS M5 log + `testing/oracle-app-harness.md`;
  M6a findings in PROGRESS M6a log + `testing/cli-e2e-pitfalls.md` (piped stdio,
  random `structurizr.dsl.identifier` UUIDs, `revision` vs `lastModified*`);
  M6b findings in PROGRESS M6b log; JSON byte rules in `agents/plan/build/
  WRITE-CATALOG.md`; keep-mode pairing rules in `crates/oxidizr-json/src/keep.rs`
  module docs; env facts in `agents/notes/env/`.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
