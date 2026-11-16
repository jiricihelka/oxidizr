# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-16 · HEAD = M6-c validate-polish commit (tip of `master`) ·
gates green · nothing pushed.

- **Done:** P1-M6c **COMPLETE**: reconcile half (commit 7351db1 — `oxidizr-cli/
  src/reconcile.rs`, identifier/canonical matching, contested-claim revoke +
  poison = OXZ-3002, geometry graft, `Renderer::note`, ambiguity-fixture trap in
  `pitfalls/reconcile-ambiguity-fixtures.md`) + validate polish (this commit —
  `suggest.rs`: `did you mean …?` in `Issue::help` at the CLI boundary, keyword
  candidates from the `Unexpected tokens (expected: …)` message, identifier
  candidates from `structurizr.dsl.identifier` + space-free names, Levenshtein
  1/2/3 budget; e2e helper tolerates EPIPE — `testing/cli-e2e-pitfalls.md` #6);
  PHASE_1 §P1-D11 M6-c DONE + PROGRESS log written (no-official-reference-
  reconcile decision recorded). P1-M6b `fmt`; P1-M6a CLI surface; P1-M5
  extends; P1-M4; infra through 2576a25.
- **Next:** **M6-d** fuzz targets + CI nightlies: `cargo fuzz init` → `fuzz/`
  (NOT yet a workspace member — decide membership + `cargo xtask` mirror,
  check WORKFLOW.md CI job table); targets at minimum `parse_dsl` (panic-free,
  all corpus extensions), `fmt` idempotence, JSON `from_slice`/`to_vec`
  round-trip (panic-free, no unbounded growth); fixture seed corpus from
  `testdata/` (Category A corpus is the seed source — see TESTING.md); CI
  nightly job time-boxed (PHASE_1 §P1-D11: "time-boxed runs"). Then **M7**:
  release-build corpus parse < 100 ms check (informational — bench job exists),
  README quickstart, tag v0.1.0.
- **Open questions:** none.
- **Don't re-derive (trust + spot-check against `ad34036`):** reconcile matching
  + fixture constraints (notes/module header); suggestion contract (PHASE_1
  §P1-D2 + suggest.rs header — CLI boundary is a DECISION, engine stays byte-
  locked); fmt line-ends `pitfalls/fmt-line-end-guards.md`; M4 findings PROGRESS
  errata + `agents/plan/DSL-ERRATA.md`; M5 `testing/oracle-app-harness.md`;
  M6a `testing/cli-e2e-pitfalls.md` (piped stdio, volatile UUIDs, EPIPE #6);
  M6b/M6c PROGRESS logs; JSON byte rules `agents/plan/build/WRITE-CATALOG.md`;
  keep-mode `crates/oxidizr-json/src/keep.rs`; env `agents/notes/env/`.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
