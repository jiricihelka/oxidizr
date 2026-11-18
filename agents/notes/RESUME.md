# RESUME — session checkpoint (updated 2026-11-18, P2T PT2 landed)

## State

- **Phase 2T in progress** (`agents/plan/build/PHASE_2T.md`) — e2e export-coverage
  interlude. Objective: implement P2T autonomously (user directive 2026-11-18);
  commit each green sub-piece, rewrite this file, **never push**, no tag at close.
- **PT1 DONE** (`9b1e7f4`): manifest v2 export sections + runner fan-out +
  5007 off the allow-list + V-T7 test + 157 sidecars blessed. Quick tier ~0.1 s.
  (PT1 also caught/fixed two pre-existing P2 bugs: `726a724`, `2cb3215`.)
- **PT2 DONE** (this commit): 7 fixtures 160–166 — installed theme (`themes/`
  dir auto-probed via the reference default source — NO runner hook needed);
  broken theme → 5008 per format ahead of 5007; unknown theme name skipped
  (**JSON-only entry** — DSL `theme <unknown>` = parse statement error, the
  load-skip branch is DSL-unreachable); terminology; autoLayout `tb|bt|lr|rl`
  positional variants; CJK/quote/bracket escapes (view keys `[a-zA-Z0-9_-]`!);
  all-8-view-kinds + 2 deployment envs (biggest clean export). 5008 OFF the
  allow-list; 5009 = permanent (probe verdict in the allow-list note).
  Plan deviations: kind×format 5007 combos don't exist; legend on/off has no
  workspace axis; theme-URL-offline already covered by 75/158.
  Also: repo litter removed (`161abfe` — stray root f1-f12.dsl from T3).
- **Next: PT3** — `tests/full_export.rs`: V-T8 pre-probe FIRST (export(dsl) vs
  export(json2json(dsl)) on 3 diverse entries — violation = real bug, witness
  verdict, fix before wiring), then determinism + path parity + fmt-fixpoint
  parity invariants (scratch replays, full tier only). Then PT4 benches +
  `bench save 2T`, PT5 close-out (TESTING.md §E2E, e2e/README, PROGRESS — no tag).
- Phase 2 shipped: tag `v0.2.0` (local, `b735732`). **Phase 3 plan FINAL**
  (`PHASE_3.md`, P3-D0 + P3-F signed off) — queued after 2T.
- Gate: `cargo xtask ci` (quartet). **Full tier is NOT in the quartet**: after
  engine changes run `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e` separately.
  Oracle 2026.10.26 (slow, weekly); mmdc absent; NO DNS.

## Workflow reminders

- Never push; commit every green sub-piece; rewrite this file each commit; gates
  before commit; JSON goldens only via oracle; reference monorepo
  `/tmp/nix-shell.uni100/opencode/structurizr` (`ad34036`, FLAT source tree);
  witness sweeps under `/tmp/nix-shell.uni100/opencode/m2d/sweep`.
- `git bisect run` needs the command directly (`env OXIDZR_E2E_FULL=1 cargo test …`) —
  `/bin/bash` is absent for shebang scripts.
- clippy `doc_markdown` flags bare Java class names in docs (backticks); prefer
  `Option<&T>` params over `&Option<T>`.
- Sidecar export keys serialize between `reconcile` and `fmt`; `bless` reuses
  `runner::observe` (no xtask change was needed for v2).
- DSL gotchas found authoring 16x: autoLayout rank dirs are `tb|bt|lr|rl`;
  view keys `[a-zA-Z0-9_-]` only; filtered views reject auto-layout bases;
  custom views include custom elements only; `deploymentEnvironment <name>`
  only; unknown theme names never parse — JSON-only for the skip branch.
