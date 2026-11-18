# RESUME — session checkpoint (updated 2026-11-18, **P2R: PR1–PR5 waves DONE**)

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
  PR4 json/api/cli wave (13 R-mis rows, BYTE-PATH) `81eb675`/`139581d`/
  `a371afc`/`1db1b5c`/`ea9ad45`/`8a67f7a` + close `679564a` (R-mis-12
  audit ratified). PR5 tooling wave (4 R-tool rows) `371a593` xtask
  (R-tool-01/02: `run_oracle_export` prologue, verify-legs `Vec<String>`,
  unwraps→context) / `42bff5b` e2e (R-tool-04: explicit feed,
  `codes_from_value`, mutate `pick_index`/`pick_line` with the rng-draw
  sequence bit-preserved) + close = R-tool-03 ratify + `cargo_cmd()`
  `expect("repo root")` elimination (xtask now has ZERO non-test
  unwrap/expect). Cumulative `bench cmp 2T` at PR4/PR5 close: ALL 23
  groups faster-or-equal (`m7_dsl_parse_big-bank-plc` 3.0→1.05 ms,
  `m7_reconcile_cross` 441→177 µs, `json_read` 1234→1046 µs,
  `json_write` 2.3→1.81 ms, `dsl2json_all` 14.6→10.4 ms).
- **Next: PR6 — the phase close** (`PHASE_2R.md` §D4 + §D6 V-R2):
  1. **V-R2 ratchet measurement**: try `redundant_clone` (nursery —
     unstable, needs a `NIGHTLY.md` register row + measured compile-time
     hit) on the post-sweep tree: `cargo clippy --workspace --all-targets
     -- -W clippy::redundant_clone` style probe; record (a) fired-site
     count, (b) compile-time delta, (c) false-positive notes → keep/drop
     decision TABLE under §D4 (table recorded even on drop). ≤ 10-allow
     kill switch (D4): more fight than value → drop.
  2. If kept: workspace-lint row in root `Cargo.toml` + NIGHTLY.md row +
     gate quartet (CI `-D warnings` makes it binding) + full tier + bless.
  3. `cargo xtask bench save 2R` + commit the baseline dir
     (`bench-baseline/2R/`, README context row).
  4. `cargo xtask oracle verify` ONCE (needs `$OXIDZR_STRUCTURIZR_BIN` —
     see `agents/notes/env/toolbox.md`; slow, Category B).
  5. Close-out docs: WORKFLOW §Code style lint row (+ R-core-13
     keep-marker notes: `builder.rs:14`/`viewops.rs:15` module-scope
     missing-doc allows = crate convention, `url.rs:113` test allow
     = 28-file convention), PROGRESS board row P2R COMPLETE + log entry,
     PHASE_2R wave table PR6 DONE, RESUME final.
  6. **No tag** (1T/2T precedent); never push.
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
- Seeded rng code dedupes only around the draw, never across it: empty
  candidate list draws NOTHING, else exactly one `rng.below` — bless
  corpus-diff=0 is then the proof the blessed `m<i>-<label>-…` dirs are
  bit-identical (PR5 mutate prologue).
- Proof convention unchanged: full tier = 8 `test result: ok`; bless =
  `corpus-diff=0`; commit via `git commit -q -F /tmp/nix-shell.v8guJ6/opencode/<msg>.txt`;
  register edits via python with `assert src.count(old) == 1`.
