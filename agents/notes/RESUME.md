# RESUME — session checkpoint (updated 2026-11-18, **P2R: PR1+PR2 waves DONE**)

## State

- **P2R active** (`agents/plan/build/PHASE_2R.md` — register + wave table are
  the boss): refactor interlude after 2T, zero behavior change, std-only,
  no new deps, **no tag**, never push. Freeze scope: contract = api
  Request/Response, CLI argv/exit, all byte outputs; pub-in-crate freely
  changeable.
- **DONE**: PR0 `21a5144` (register, 65 rows). PR1 core wave `2359084`/
  `cb9cbdd`/`2b8d43a`/`1b90568` + close `3a6a4eb` (all R-core rows). PR2 dsl
  wave `c09bbcd`/`0d8026a`/`6cc507f`/`63cbc5a`/`fd3c257` (all R-dsl rows:
  08 DF→LSP/P4, rest DONE; bench cumulative: dsl_parse_big-bank 3.0→1.07 ms,
  reconcile_cross 441→186 µs, dsl2json_all 14.6→10.2 ms, ALL groups
  faster-or-equal vs `2T`).
- **Next: PR3 export wave** (`oxidizr-export`): R-exp-01..04/06/07/13/14 are
  the small-SW cluster; R-exp-05 `IndentingWriter` needs its OWN commit +
  bench (bytes guarded by the whole export net); R-exp-08 memoize is
  criterion-verifiable; R-exp-09 pub shrink (only `ExportFormat`/
  `ExportError`/`ThemeStore`/`ThemeError`/`ExportedFile` cross the crate);
  R-exp-10/11 splits keep fn names (cited in `EXPORT-MERMAID.md:141,146`);
  R-exp-15/17-traits KC/DF. Wave ritual per commit: `cargo xtask ci` +
  `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e` (8 `test result: ok`) +
  `cargo xtask e2e bless` zero-diff (`corpus-diff=$(git status --short e2e/ | wc -l)` = 0).
- Then PR4 (json/api/diag/ast/cli), PR5 (xtask/e2e harness), PR6 (§D4 ratchet
  + `bench save 2R` + `oracle verify` once + close-out docs; PROGRESS board
  row + WORKFLOW lint row).
- Gate: `cargo xtask ci` (quartet). **Full tier is NOT in the quartet**: after
  engine changes run `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e` separately.
  Oracle 2026.10.26 (slow, weekly); mmdc absent; NO DNS.
- CI legs to watch on the next push: ci.yml **windows** leg (PT3b fix) and
  nightly `e2e.yml` full tier (PT3 invariants). Local quartet + full tier green.

## 2R refactor patterns that worked

- R-dsl-06 shape: guard-only `p.top().cloned()` preludes → fresh `p.top()`
  reads per guard/arm (safe: every mutating arm returns); payload arms clone
  ONLY the matched field; arms binding through the `p.top()` borrow
  materialize Copy payloads (`let view = *view;`) before any `&mut p` use —
  including as the first line inside `||` closures that capture `p`.
- Cow tokens (`Tokens::get` → `Cow<'_, str>`): sinks `.into_owned()`, `&str`
  consumers keep the borrow via deref coercion; `match x.as_str()` →
  `match &*x`; compare-only sites `Tokens::eq_at`.
- Dead code surfaced by `pub(crate)` demotions: keep Java-parity API +
  `#[allow(dead_code)]` + per-item rationale; delete port-introduced waste
  (R-core-06 policy).

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
- Python sweep caveats (PR2 lesson): assert per-pattern counts; escape `||` in
  regexes; eyeball parens after `\)\)\)`-style edits; compiler as the boss.
- DSL gotchas found authoring 16x: autoLayout rank dirs are `tb|bt|lr|rl`;
  view keys `[a-zA-Z0-9_-]` only; filtered views reject auto-layout bases;
  custom views include custom elements only; `deploymentEnvironment <name>`
  only; unknown theme names never parse — JSON-only for the skip branch.
- Export benches select the biggest entry by manifest plantuml **file count**
  (166), not DSL text size; corpus growth renames `*_biggest_*` ids by design.
