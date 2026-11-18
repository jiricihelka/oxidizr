# RESUME — session checkpoint (updated 2026-11-18, **Phase 2T COMPLETE**)

## State

- **Phase 2T COMPLETE** (`agents/plan/build/PHASE_2T.md`, PT1–PT5 all DONE).
  e2e export-coverage interlude, no tag (1T precedent), nothing pushed.
  Commits: `9b1e7f4` (PT1 manifest v2 + fan-out + V-T7 + 157 blessed sidecars;
  caught pre-existing P2 bugs `726a724` reconcile default-views tail +
  `2cb3215` json2json byte-fixpoint invariant), `161abfe` (repo litter),
  `ffc3011` (PT2 fixtures 160–166, 5008 off the allow-list), `37fca46` (PT3b
  Windows `oxidizr.exe` resolution — windows ci.yml leg was latently red since
  P1T), `12ad7d0` (PT3 `tests/full_export.rs` invariants, V-T8 closed 9/9),
  `7296929` (PT4 export benches + `2T` baseline), PT5 close-out (this commit:
  TESTING.md §E2E, e2e/README, PROGRESS board + log, plan DONE rows).
- **Next: Phase 3** — plan FINAL (`agents/plan/build/PHASE_3.md`, P3-D0 + P3-F
  signed off): viewer + layout. Start from its T0/M0 entry; board row `P3` in
  PROGRESS.md is `pending`.
- Phase 2 shipped: tag `v0.2.0` (local, `b735732`).
- Gate: `cargo xtask ci` (quartet). **Full tier is NOT in the quartet**: after
  engine changes run `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e` separately.
  Oracle 2026.10.26 (slow, weekly); mmdc absent; NO DNS.
- CI legs to watch on the next push: ci.yml **windows** leg (PT3b fix) and
  nightly `e2e.yml` full tier (PT3 invariants). Local quartet + full tier green.

## Workflow reminders

- Never push; commit every green sub-piece; rewrite this file each commit; gates
  before commit; JSON goldens only via oracle; reference monorepo
  `/tmp/nix-shell.uni100/opencode/structurizr` (`ad34036`, FLAT source tree);
  witness sweeps under `/tmp/nix-shell.uni100/opencode/m2d/sweep`.
- **Windows CI leg runs `cargo test --workspace`** (ci.yml test matrix) — any
  test shelling out to the `oxidizr` binary MUST resolve `oxidizr.exe` too
  (subprocess_smoke fixed PT3b); no `#[cfg(unix)]`-only binary paths.
- `git bisect run` needs the command directly (`env OXIDZR_E2E_FULL=1 cargo test …`) —
  `/bin/bash` is absent for shebang scripts.
- clippy `doc_markdown` flags bare Java class names in docs (backticks); prefer
  `is_ok_and` over `map_or(false, …)`.
- Sidecar export keys serialize between `reconcile` and `fmt`; `bless` reuses
  `runner::observe` (no xtask change was needed for v2).
- DSL gotchas found authoring 16x: autoLayout rank dirs are `tb|bt|lr|rl`;
  view keys `[a-zA-Z0-9_-]` only; filtered views reject auto-layout bases;
  custom views include custom elements only; `deploymentEnvironment <name>`
  only; unknown theme names never parse — JSON-only for the skip branch.
- Export benches select the biggest entry by manifest plantuml **file count**
  (166), not DSL text size; corpus growth renames `*_biggest_*` ids by design.
