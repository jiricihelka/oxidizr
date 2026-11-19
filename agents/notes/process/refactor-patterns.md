# Refactor patterns that held during P2R (zero-behavior-change waves)

Distilled 2026-11-19 from the 2R sweep waves (waves + hashes: PROGRESS log "P2R").

- clippy `unnecessary_wraps` is the oracle for phantom Results — do the infallible
  cascade (dropping the two phantom wrappers made `model()` infallible too), never
  `#[allow]` it away.
- `?`-in-closure collects need the error type spelled:
  `collect::<Result<Vec<_>, JsonError>>()?` (rustc can't commit it through the two-hop `?`).
- Return-and-flatten beats out-param threading, but preserve the PUSH ORDER exactly
  when a stable sort dedups downstream.
- Result-shape ladders: carry the partial log INSIDE the error arm (`Fail(Outcome)`) —
  `?` at the frozen boundary then reproduces the old `out.fail(…); return out` bytes.
- `set_list`-style helpers: infallible is the default; fallibility lives in the name
  (`try_set_list`).
- `entry().or_insert()` keeps the FIRST entry per id — the exact answer an
  `iter().find(…)` scan gave.
- Seeded rng code dedupes only AROUND the draw, never across it: empty candidate list
  draws NOTHING, else exactly one `rng.below` — bless `corpus-diff=0` is then the
  proof the blessed `m<i>-<label>-…` dirs are bit-identical.
- A by-value param + exclusive match arms: each arm can MOVE it (6× `group.clone()`
  → move killed `needless_pass_by_value`'s allow with it) — `redundant_clone` flags
  exactly this family.
- Proof conventions that made waves safe: bless no-op = whole-net behavior diff
  (run it once on the UNMODIFIED tree first — V-R1 — to prove the harness);
  full tier = 8 `test result: ok`; register edits via python
  `assert src.count(old) == 1`.

Last useful: 2026-11-19
