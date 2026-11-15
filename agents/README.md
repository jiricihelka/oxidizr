# agents/ — permanent agent working files

Everything a future session should know that is *not* code, plus the design
history. Read the README in each subdirectory first; drill into files only as
needed (they are small on purpose — minimize context pollution).

- `notes/` — short operational memory (env setup, pitfalls, testing rules,
  process). Start at `notes/README.md`. **Write a note after every non-obvious
  fix** — that is the whole point.
- `plan/` — the authoritative design corpus (moved here from top-level `plan/`
  on 2026-11-15): the compatibility contract (`COMPAT.md`), distilled Structurizr
  reference, decision registers (`DEVIATIONS.md` locked list!), workflow/testing
  policy (`WORKFLOW.md`, `TESTING.md`), per-phase build plans, `PROGRESS.md`
  milestone log/errata. `agents/plan/README.md` gives the index and reading order;
  AGENTS.md §Where things are is the charter pointer.
- `plan/reports/` — user-dropped CI/OS failure reports (untracked scratch).

Fast search: `grep -ri <term> agents/notes` (notes are one screen each).
