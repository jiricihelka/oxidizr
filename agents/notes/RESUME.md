# RESUME — session checkpoint (updated 2026-11-18, P2T PT3 + PT3b landed)

## State

- **Phase 2T in progress** (`agents/plan/build/PHASE_2T.md`) — e2e export-coverage
  interlude. Objective: implement P2T autonomously (user directive 2026-11-18);
  commit each green sub-piece, rewrite this file, **never push**, no tag at close.
- **PT1 DONE** (`9b1e7f4`): manifest v2 export sections + runner fan-out +
  5007 off the allow-list + V-T7 test + 157 sidecars blessed (quick tier ~0.1 s;
  it caught/fixed two pre-existing P2 bugs: `726a724`, `2cb3215`).
  **PT2 DONE** (`ffc3011`, fixtures 160–166, 5008 off the allow-list; repo litter
  gone `161abfe`).
- **PT3 DONE** (this commit): `tests/full_export.rs` — determinism + path parity
  (`export(dsl) ≡ export(json2json(dsl))`) + fmt-fixpoint parity, per entry × 3
  landed formats over all 163 export-applicable entries; JSON/formatted replays
  run from a scratch COPY of the entry dir (children + themes/). V-T8 pre-probed
  green (9/9 byte-exact parity probes) BEFORE wiring. Full-tier env gate only.
- **PT3b DONE** (this commit): Windows CI fix (user-reported) —
  `subprocess_smoke::binary()` now tries BOTH `oxidizr` and `oxidizr.exe` at
  every ancestor (cfg-free). The `windows-latest` ci.yml leg was LATENT RED
  since P1T (bin there is `oxidizr.exe`); the 2T CI run surfaced it.
  `$OXIDZR_BIN` still wins.
- **Next: PT4** — corpus export benches `e2e_export_{plantuml,plantuml_c4,mermaid}_all`
  behind the `e2e-bench` feature + `cargo xtask bench save 2T` baseline commit
  (CI bench job stays feature-free). Then PT5 close-out: TESTING.md §E2E
  (manifest v2 + export matrix), e2e/README, PROGRESS board + log, PHASE_2T DONE
  entries — **no tag**. A dispatched `e2e.yml` full-tier run is the acceptance leg.
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
- **Windows CI leg runs `cargo test --workspace`** (ci.yml test matrix) — any
  test shelling out to the `oxidizr` binary MUST resolve `oxidizr.exe` too
  (subprocess_smoke fixed in PT3b); no `#[cfg(unix)]`-only binary paths.
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
