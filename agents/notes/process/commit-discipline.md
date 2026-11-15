# commit discipline

- Gates before EVERY commit, in order: `cargo fmt --all --check` →
  `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace`
  (or the mirror `cargo xtask ci`). Never commit red.
- One commit per green milestone/unit of work; message style: conventional prefix +
  scope (`feat(xtask):`, `fix(ci):`, `infra(dev):`, `ciinfra:`, `chore(testdata):`,
  `docs(plan):`) with a body that states WHY + verification performed.
- **Never push** — local commits only unless the user says otherwise.
- Compatibility failures are fixed in oxidizr, never by editing expectations/goldens
  (AGENTS.md §Compatibility rules). Goldens regenerate only via `cargo xtask oracle`.
- Windows report files land in `agents/plan/reports/` (untracked) — read, fix, leave.
