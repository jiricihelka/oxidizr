# rust-version bumps fire a clippy lint wave (P5R-R0, 2026-11-25)

Symptom: quartet green yesterday, `collapsible_if` errors in ~25 places
today with the toolchain pin and every source file unchanged.

Cause: `cargo clippy` passes `--target-version` from the manifest's
`rust-version`. Raising it 1.85 → 1.88 enabled lints gated on newer stable
APIs — `collapsible_if` starts demanding let-chain collapses, and a
`chunks_exact(n)` → `as_chunks::<n>()` lint appears (that method stabilized
in 1.88). Nothing flaked; the MSRV edit IS a clippy config change.

Fix procedure that worked: review-by-hand the first crate's sites (message
text, `continue` semantics inside `if`), then
`cargo clippy --fix --workspace --all-targets --allow-dirty` (machine-safe:
the lint only fires when the inner `if` is the SOLE statement, so the
collapse short-circuits identically), `cargo fmt --all`, then audit the WHOLE
diff hunk-by-hunk — the fixer also touched crates clippy had not reached
before the first error stopped the run, and comments survived.

Rule: any `rust-version` bump lands together with its lint-wave fixes in the
same commit, or bisect goes red.
