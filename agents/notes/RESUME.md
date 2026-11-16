# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-16 · HEAD = M6-c reconcile commit (tip of `master`) ·
gates green · nothing pushed.

- **Done:** P1-M6c **reconcile half** (`crates/oxidizr-cli/src/reconcile.rs`):
  old→new id maps (`structurizr.dsl.identifier` exact → canonical-name fallback
  over `adopt`ed builder clones), contested claims = OXZ-3002 revoked-never-guessed
  (fixtures must dodge the parse/hydrate duplicate-name checks —
  `pitfalls/reconcile-ambiguity-fixtures.md`), geometry graft (x/y, routing,
  paper/dimensions/automaticLayout, remapped animations), JSON emitted even when
  ambiguous, report on stderr via `Renderer::note`, exit 1 only on ambiguity;
  CLI arm wired (bail test → 2 e2e tests, e2e 20→21 + 8 unit);
  P1-M6b `fmt`; P1-M6a CLI surface; P1-M5 extends; P1-M4; infra through 2576a25.
- **Next:** finish **M6-c validate polish**: did-you-mean suggestions into
  `Issue::help` (`DslError` needs a `suggestion` field; candidates: OXZ-2001
  `catch_all`/`permitted_tokens_message` sites in `engine/mod.rs`, the
  `"{} could not be found"` family ~`engine/mod.rs:1346`) → commit → PHASE_1
  §P1-D11 **M6-c DONE** + PROGRESS entry (record the no-official-reference-
  reconcile-tool decision: structurizr org has none — P1-D8 text is authority).
  Then **M6-d** fuzz targets (`cargo-fuzz` installed, `fuzz/` not created yet) +
  CI nightlies, then **M7** (release corpus < 100 ms check, README quickstart,
  v0.1.0 tag).
- **Open questions:** none.
- **Don't re-derive (trust + spot-check against `ad34036`):** reconcile matching
  rules + fixture constraints in the reconcile notes/module header; fmt line-end
  contract in `pitfalls/fmt-line-end-guards.md`; M4 findings in PROGRESS errata +
  `agents/plan/DSL-ERRATA.md`; M5 findings in PROGRESS M5 log +
  `testing/oracle-app-harness.md`; M6a findings in `testing/cli-e2e-pitfalls.md`
  (piped stdio, volatile identifier UUIDs, `revision` vs `lastModified*`);
  M6b/M6c findings in PROGRESS logs; JSON byte rules in
  `agents/plan/build/WRITE-CATALOG.md`; keep-mode rules in `crates/oxidizr-json/
  src/keep.rs`; env facts in `agents/notes/env/`.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
