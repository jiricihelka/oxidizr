# RESUME — session checkpoint (updated 2026-11-18, **P2R: PR1–PR3 waves DONE**)

## State

- **P2R active** (`agents/plan/build/PHASE_2R.md` — register + wave table are
  the boss): refactor interlude after 2T, zero behavior change, std-only,
  no new deps, **no tag**, never push. Freeze scope: contract = api
  Request/Response, CLI argv/exit, all byte outputs; pub-in-crate freely
  changeable.
- **DONE**: PR0 `21a5144`. PR1 core wave `2359084`/`cb9cbdd`/`2b8d43a`/
  `1b90568` + close `3a6a4eb`. PR2 dsl wave `c09bbcd`/`0d8026a`/`6cc507f`/
  `63cbc5a`/`fd3c257` + close `89989d6` (all R-dsl rows). PR3 export wave
  `b1b295e`/`a64da54` (+ bench record `041ff22`)/`7bfbac8`/`8a1e763`/
  `70ccde4`/`daf8e24`/`307518b` + close (KC ratifications R-exp-12/15,
  container-leg cite) — all 18 R-exp rows dispositioned. Cumulative
  `bench cmp 2T` at PR3 close: ALL 23 groups faster-or-equal, export legs
  1.37–1.92× (`plantuml_all` 17.9→13.0 ms, `c4_biggest_166` 487→256 µs),
  `dsl2json_all` 14.6→10.2 ms, `reconcile_166` 1146→597 µs.
- **Next: PR4** (`oxidizr-json` + api + diag + ast + cli — 13 `R-mis` rows,
  BYTE-PATH: writer/printer legs cite D2 rules; full tier + bless per commit):
  json cluster = R-mis-02 (`deployment_instance_tail` args + allow), 03
  (dynamic rel-views sort: precompute keys, **preserve stable-sort ORDER** —
  witness `7/1, 9/2, 10/3, 11/3`), 06 (printer dead-identical scalar-vs-
  object array arms — verified equal, collapse + Jackson comment), 07
  (reader `rels` out-param → return-and-flatten), 08 (`theme_to_vec` deep
  clones), 09 (`set_list(…).unwrap_or(None)` → infallible variant, ×13),
  13-writer; api/cli cluster = R-mis-01 (`Outcome` accumulator → helper
  Result at the frozen `execute` boundary, partial-log semantics carried),
  04 (suggest tuple-bool → `&&`), 05 (parse-error decoration ladder ×4 →
  one helper), 10 (`to_request` clones a dead `Cli` → move), 11 (reconcile
  `rels: bool` ×2 + shadowed closure + scans); R-mis-12 = KC audit-trail.
- Then PR5 (`R-tool` 4 rows: xtask + e2e harness), PR6 (§D4 ratchet
  + `bench save 2R` + `oracle verify` ONCE + close-out docs: PROGRESS board
  row + WORKFLOW lint row; **no tag**).
- Wave ritual per commit: `cargo xtask ci` +
  `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e` (8 `test result: ok`) +
  `cargo xtask e2e bless` zero-diff (`corpus-diff=$(git status --short e2e/ | wc -l)` = 0).
- Gate: `cargo xtask ci` (quartet). **Full tier is NOT in the quartet**: after
  engine changes run the full tier separately. Oracle 2026.10.26 (slow, weekly);
  mmdc absent; NO DNS.
- CI legs to watch on the next push: ci.yml **windows** leg (PT3b fix) and
  nightly `e2e.yml` full tier (PT3 invariants). Local quartet + full tier green.

## 2R refactor patterns that worked

- R-dsl-06 shape: guard-only `p.top().cloned()` preludes → fresh `p.top()`
  reads per guard/arm; payload arms clone ONLY the matched field; Copy
  payloads materialize (`let view = *view;`) before any `&mut p` use.
- Cow tokens (`Tokens::get` → `Cow<'_, str>`): sinks `.into_owned()`, `&str`
  consumers keep the borrow; `match x.as_str()` → `match &*x`; compare-only
  `Tokens::eq_at`.
- Dead code surfaced by `pub(crate)` demotions: keep Java-parity API +
  `#[allow(dead_code)]` + rationale; delete port-introduced waste (R-core-06).
- Memoizing over the `'a` workspace: lazy `OnceCell`/`RefCell<HashMap<&'a str, _>>`
  — but a `HashMap` KEY makes the host struct **invariant** in `'a`: helpers
  taking elided `&Engine` + `View<'_>` then fail (tie both to one named `'a`),
  and `HashMap::new` is not `const` → the `const fn` ctor must go (PR3-d).
- `let … else` with a top-level or-pattern needs parentheses around the
  pattern; field-less style legs become Self-less assoc fns (pedantic
  `unused_self`); bare kind/class names in docs need backticks (`doc_markdown`).
- Disjoint-field borrows beat whole-Vec clones: `load_themes` iterates
  `configuration.themes` while appending to the SIBLING `styles.themes`.

## Workflow reminders

- Never push; commit every green sub-piece; rewrite this file at wave closes;
  gates before commit; JSON goldens only via oracle; reference monorepo
  `/tmp/nix-shell.uni100/opencode/structurizr` (`ad34036`, FLAT source tree);
  witness sweeps under `/tmp/nix-shell.uni100/opencode/m2d/sweep`.
- **Windows CI leg runs `cargo test --workspace`** — any test shelling out to
  the `oxidizr` binary MUST resolve `oxidizr.exe` too; no `#[cfg(unix)]`-only
  binary paths.
- `git bisect run` needs the command directly (`env OXIDZR_E2E_FULL=1 cargo test …`).
- clippy `doc_markdown` flags bare Java class names (backticks); prefer
  `is_ok_and` over `map_or(false, …)`.
- Python sweep caveats: assert per-pattern counts; escape `||` in regexes;
  compiler as the boss.
- Bench logs: `cargo xtask bench` then `cargo xtask bench cmp 2T`; the cmp
  ratio is 2T/new — >1.00 means faster. Baseline dir `bench-baseline/2T/`.
