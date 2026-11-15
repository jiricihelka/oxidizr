# RESUME.md — session checkpoint

Rewritten at every green commit (ritual: `process/resume-ritual.md`). Newest wins;
never archive — history is git's job.

Updated: 2026-11-15 · HEAD `6bb10e8` · gates green · nothing pushed.

- **Done:** P1-M4 (engine + burn-in, green on host AND Wine `ci windows`);
  post-M4 infra (Wine leg, CI caches, bench-job fix, `cargo xtask ci|bench|cov`,
  `agents/` reorg, charter slimming); workflow tooling: dev-shell triage tools
  (jq/sqlite/python3/actionlint/critcmp + ast-grep w/ search recipes note),
  `cargo xtask refs` = 4th default gate
  (quartet fmt+clippy+test+refs), `ci lint` step + CI `lint` job (actionlint
  1.7.12 pinned+sha256), `bench save <LABEL>` / `bench cmp <LABEL>` (critcmp
  staging verified vs M0), `wineserver -k` after the windows leg.
- **Next:** P1-M5 directives — `!include` (file/dir), `!const`/`!var` + `${}`,
  `scope shared`, `workspace extends` (file only), `!identifiers`, finders,
  `!docs`/`!adrs` statement-form + OXZ-5003. Scope: PHASE_1.md §M5. Decompose
  before starting (size gate); commit each green sub-piece; retire the `M5`
  `KNOWN_ERRORS` rows as pieces land (testing/burn-in-register.md).
- **Open questions:** none.
- **Don't re-derive (trust + spot-check against `ad34036`):** M4 findings in
  `agents/plan/PROGRESS.md` errata + `agents/plan/DSL-ERRATA.md`; JSON byte rules
  in `agents/plan/build/WRITE-CATALOG.md`; env facts in `agents/notes/env/`;
  critcmp takes only in-root baseline names (`bench cmp` stages, see main.rs doc).
- **Cadence:** `cargo xtask ci` green → commit + rewrite this file; never push.
