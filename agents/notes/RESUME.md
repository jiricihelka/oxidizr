# RESUME — session checkpoint (updated 2026-11-18, **P2R: PR1–PR4 waves DONE**)

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
  `70ccde4`/`daf8e24`/`307518b` + close `a3d73ed` (all 18 R-exp rows).
  PR4 json/api/cli wave (13 R-mis rows, BYTE-PATH): PR4-a `81eb675` json
  writer cluster (R-mis-02/03/08/09/13 — infallible `set_list`/`list` +
  `try_set_list`, `decisions` swallow KEPT as byte contract w/ hazard
  comment, `InstanceTail`, `RvSortKey`, `ThemeView`), PR4-b1 `139581d`
  printer arms (R-mis-06), PR4-b2 `a371afc` reader return-and-flatten
  (R-mis-07), PR4-c1 `1db1b5c` pipeline Result shape (R-mis-01/05: private
  `Fail(Outcome)` flattened at the frozen `execute`; `statement_errors`
  helper ×4), PR4-c2 `ea9ad45` `decorate` `&&` + `to_request`/`replay`
  moves (R-mis-04/10), PR4-c3 `8a67f7a` reconcile named builders + set
  lookups (R-mis-11); PR4-d = R-mis-12 audit-trail ratification (no code:
  javahash IS the byte contract, cites verified). Cumulative `bench cmp 2T`
  at PR4 close: ALL 23 groups faster-or-equal — json byte-path legs
  `json_read` 1234→1087 µs / `json_write` 2.3→1.96 ms, `dsl2json_all`
  14.6→9.7 ms, `validate_all` 11.0→6.5 ms, `reconcile_166` 1146→567 µs,
  `c4_biggest_166` 487→255 µs.
- **Next: PR5** (`R-tool` 4 rows — `cargo xtask` + `oxidizr-e2e` harness,
  the least constrained wave; tooling code, quartet + full tier + bless
  still per commit):
  - R-tool-01: 3× oracle-runner triplication (`reference_export_json`/
    `_plantuml`/`_theme`, `xtask main.rs:685-784`) → shared
    `run_oracle_export(oracle, workspace, outdir, format) -> Result<PathBuf/
    Vec, String>`; `verify_json_legs`/`verify_diagram_legs`/`verify_theme_legs`
    (`:1186-1310`) `failures: &mut usize` out-params → return
    `Vec<String>` failure lines, `cmd_oracle_verify` folds them.
  - R-tool-02: `unwrap()` ×2 in NON-test xtask (`main.rs:418`
    `p.file_name().unwrap()` in bench-save, `:487` same in restore) →
    `with_context`/`?` (charter: unwrap only in tests).
  - R-tool-03: oracle_gen bool-flag ladder (`main.rs:1059-1096`) —
    validated mutually exclusive; **ratify keep** (KC row, no code).
  - R-tool-04: e2e triple — `runner.rs:63` `unwrap_or_default()` double
    swallower on the dsl2json feed; `manifest.rs:400/462` codes-parse
    duplicated ×2 → one `codes_from_value` helper; `mutate.rs:67-153`
    5× op prologue (split→cands filter→empty→rng pick) → shared helper,
    **keep the seeded index selection byte-identical** (mutation dirs are
    blessed corpus names — `m119-drop-brace-64-…`).
  - R-tool-03's ratification + the wave-row DONE + RESUME = PR5 close.
- Then PR6 (§D4 ratchet measurement + enable kept lints; `bench save 2R`;
  `oracle verify` ONCE; close-out docs: PROGRESS board row, WORKFLOW §Code
  style lint row, PHASE_2R DONE, RESUME; **no tag**).
- Wave ritual per commit: `cargo xtask ci` +
  `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e` (8 `test result: ok`) +
  `cargo xtask e2e bless` zero-diff (`corpus-diff=$(git status --short e2e/ | wc -l)` = 0).
- Gate: `cargo xtask ci` (quartet). **Full tier is NOT in the quartet**: after
  engine changes run the full tier separately. Oracle 2026.10.26 (slow, weekly);
  mmdc absent; NO DNS.

## 2R refactor-pattern notes (learned in this phase)

- clippy `unnecessary_wraps` is the oracle for phantom Results — do the
  infallible cascade (PR4-a: dropping the two phantom wrappers made
  `model()` infallible too), never `#[allow]` it away.
- `?`-in-closure collects need the error type spelled:
  `collect::<Result<Vec<_>, JsonError>>()?` (rustc can't commit it through
  the two-hop `?`).
- Return-and-flatten beats out-param threading, but preserve the PUSH
  ORDER exactly when a stable sort dedups downstream (PR4-b2).
- Result-shape ladders: carry the partial log INSIDE the error arm
  (`Fail(Outcome)`), so `?` at the frozen boundary reproduces the old
  `out.fail(…); return out` byte-for-byte (PR4-c1).
- `set_list`-style helpers: infallible is the default; fallibility lives
  in the name (`try_set_list`).
- `entry().or_insert()` keeps the FIRST entry per id — the exact answer an
  `iter().find(…)` scan gave (PR4-c3).
- Proof convention unchanged: full tier = 8 `test result: ok`; bless =
  `corpus-diff=0`; commit via `git commit -q -F /tmp/nix-shell.v8guJ6/opencode/<msg>.txt`;
  register edits via python with `assert src.count(old) == 1`.
