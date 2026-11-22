# P4T fuzz finds and surface bounds (T-5)

The seven-target battery (`cargo xtask fuzz`) found two REAL engine bugs —
never fix such finds by weakening the target.

- **fmt BOM idempotence** (`dsl_fmt` crash → `48aa18b`): a BOM after the
  indent is a TOKEN character (reference `Tokenizer` trims, `isWhitespace`
  doesn't split U+FEFF), so re-indenting to column 0 lands it at byte 0 where
  `StructurizrDslParser.java:249` strips it per line → `fmt∘fmt ≠ fmt`. Fix =
  protective leading SPACE for a BOM-led body at `shown == 0` (line-START
  mirror of the `\`/`"""` end guards, see fmt-line-end-guards.md).
- **pathfilter zero-width doubling** (timeout → `0e8d7f3`): CPA matcher legs
  called the continuation TWICE per empty-width iteration (closure `k(j)` +
  trailing `k(i)`), so `$+$+…` was 2^n; the JDK counts one empty iteration
  then stops repetition (probe: `(a*)+` on `""` → true, `(?:a*)+b` on `b` →
  true, no hang). Each continuation position must be tried exactly once.
- **Slow-unit backtracking is PARITY** — `java.util.regex` is unbounded too
  (`.+.+\D+\D+…`); defense = bounded FUZZ surface (fixed ≤4-char subjects,
  pattern ≤256), never a matcher cap.
- The `fuzz/` workspace is OUTSIDE the quartet: `ParseOptions` field drift
  (`installed_themes`) broke the old targets silently — `cargo fuzz build`
  is its only gate; run `cargo xtask fuzz N` before touching fuzz targets.
- Single-input replay: `ASAN_OPTIONS=detect_odr_violation=0
  fuzz/target/x86_64-unknown-linux-gnu/release/<t> -runs=1 <artifact>`.

Last useful: 2026-11-22
