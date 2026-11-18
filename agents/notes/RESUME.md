# RESUME — session checkpoint (updated 2026-11-18, P2T PT1 landed)

## State

- **Phase 2T in progress** (`agents/plan/build/PHASE_2T.md`) — e2e export-coverage
  interlude. Objective: implement P2T autonomously (user directive 2026-11-18);
  commit each green sub-piece, rewrite this file, **never push**, no tag at close.
- **PT1 DONE** (this commit): manifest v2 export sections (`plantuml`,
  `plantuml-c4`, `mermaid`, `websequencediagrams` compat probe: exit + codes +
  `files` count), runner fan-out into scratch dirs, coverage gate collects export
  codes (5007 OFF the allow-list), `tests/path_leakage.rs` (V-T7 permanent),
  full bless = 157 sidecars gained export sections (155 × 5007, 2 × 5001).
  Quick tier ~0.1 s; full tier green.
- **PT1 caught + fixed two PRE-EXISTING P2 regressions** (bisected to `cc243a0`):
  reconcile missing the default-views load tail (`726a724`), and the json2json
  invariant now a byte FIXPOINT `out₃==out₂` (`2cb3215`, WRITE-CATALOG §5
  TreeSet-rebuild contract — `out₂==out₁` is false BY CONTRACT, oracle-witnessed).
- **Next: PT2** — ~10 new `16x` export fixtures (plan D3 list: deployment/dynamic
  in export position, filtered view, legend on/off pair, `automaticLayout` header
  variants, deep styles, `themes/` installed-theme dir fixture, theme URL offline,
  broken theme FILE → cover 5008, unknown theme name, CJK/escaping names). Probe
  whether 5009 is corpus-reachable, else keep allow-listed. Numbers continue
  after 159. Then PT3 `tests/full_export.rs` (V-T8 pre-probe 3 entries FIRST),
  PT4 benches + `bench save 2T`, PT5 close-out (TESTING.md §E2E, e2e/README,
  PROGRESS — no tag).
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
