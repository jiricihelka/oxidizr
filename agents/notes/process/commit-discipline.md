# commit discipline

- Gates before EVERY commit, in order: `cargo fmt --all --check` →
  `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace`
  (or the mirror `cargo xtask ci`). Never commit red.
- **Commit cadence (revised 2026-11-15 after M4): every green sub-piece gets its
  own commit** — never hold a day of work uncommitted (context-compaction recovery
  is the cost, see resume-ritual.md). A milestone finishes with a wrap-up commit
  (PROGRESS.md log + status board). Rewrite `agents/notes/RESUME.md` at each
  green commit.
- Message style: conventional prefix + scope (`feat(xtask):`, `fix(ci):`,
  `infra(dev):`, `ciinfra:`, `chore(testdata):`, `docs(plan):`) with a body that
  states WHY + verification performed.
- **Never push** — local commits only unless the user says otherwise.
- Compatibility failures are fixed in oxidizr, never by editing expectations/goldens
  (agents/plan/COMPAT.md). Goldens regenerate only via `cargo xtask oracle`.
- Windows report files land in `agents/plan/reports/` (untracked) — read, fix, leave.
