# faithful-port rules (engine vs Java)

The DSL engine/serializer are ports of the pinned structurizr Java (monorepo
ad34036) — behavior matches the REFERENCE, not idiomatic Rust. Consequences:

- Lint deviations from clippy `-D warnings` need an inline `#[allow(...)]` with a
  one-line rationale tying it to the Java original (e.g. `clippy::assigning_clones`
  in `oxidizr-json/src/javahash.rs`). Add the allow at the narrowest scope.
- Java behaviors that look like bugs are reproduced (captured in
  agents/plan/DSL-ERRATA.md / JSON-FORMAT.md); if the reference itself is suspect,
  write the divergence up in `agents/plan/` BEFORE deviating, and check the
  DEVIATIONS.md Locked list first — those items never deviate.
- No `unsafe`, std-only, no platform cfgs without a justifying comment; errors as
  values (`thiserror` in libs, `anyhow` in CLI/xtask).
- Perf claims need criterion numbers; >~10% regression vs committed baseline is
  reviewed (`cargo xtask bench` + critcmp).
