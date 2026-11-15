# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-15 · HEAD `5fd4015` · gates green · nothing pushed.

- **Done:** P1-M4 (engine + burn-in, green on host AND Wine `ci windows`);
  post-M4 infra (Wine leg, CI caches, bench-job fix, `cargo xtask ci|bench|cov`,
  `agents/` reorg, charter slimming).
- **Next:** P1-M5 directives — `!include` (file/dir), `!const`/`!var` + `${}`,
  `scope shared`, `workspace extends` (file only), `!identifiers`, finders,
  `!docs`/`!adrs` statement-form + OXZ-5003. Scope: PHASE_1.md §M5. Decompose
  before starting (size gate); commit each green sub-piece; retire the `M5`
  `KNOWN_ERRORS` rows as pieces land (testing/burn-in-register.md).
- **Open questions:** none.
- **Don't re-derive (trust + spot-check against `ad34036`):** M4 findings in
  `agents/plan/PROGRESS.md` errata + `agents/plan/DSL-ERRATA.md`; JSON byte rules
  in `agents/plan/build/WRITE-CATALOG.md`; env facts in `agents/notes/env/`.
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
