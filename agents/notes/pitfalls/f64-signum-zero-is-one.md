# `f64::signum(0.0) == 1.0` — side-of-line tests must hand-roll the sign

Measured 2026-11-28 in the P7 layout-quality metric
(`crates/oxidizr-layout/tests/layout_quality.rs`).

* The classic crossing test is `side(a.from,b) * side(a.to,b) < 0` with
  `side = cross.signum()`. Rust's `signum` maps **`+0.0` to `1.0`** (IEEE
  keeps the sign bit; `-0.0` gives `-1.0`) — so a route that merely TOUCHES
  another route's endpoint, or runs collinear through it, counted as a full
  crossing. Every table number was inflated, quietly and on both engines.
* Fix: hand-rolled sign (`>0 ⇒ 1`, `<0 ⇒ -1`, else `0`) + metric self-tests
  (X crosses 1; parallel lanes 0; shared endpoint 0; touching legs 0).
* Generalize: any orientation/classification predicate that can land exactly
  on zero (geometry metrics, collinearity, dedup-by-cross-product) needs the
  three-way hand-roll — `partial_cmp(...).unwrap_or(Equal)` style, not
  `signum`.
* The blessed corpus numbers pre-fix are NOT comparable to post-fix ones;
  the table was re-blessed in the same commit as the fix.

Last useful: 2026-11-28
