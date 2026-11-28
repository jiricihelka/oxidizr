# `cargo xtask ci <STEP…>` REPLACES the default quartet — it never extends it

**Trap:** naming any step drops the defaults. `cargo xtask ci gui` runs
exactly ONE step — the gui clippy leg — and reports "ci: all steps green"
after it. The workspace `test` leg never runs, so the self-gating tests
inside xtask (`refs::tests::shipped_tree_is_hygiene_clean`) never execute.

**Paid at P6-M8 close (2026-11-28):** waves M5–M7 gated with the bare
`ci gui` form (plus per-crate `cargo test -p oxidizr-viewer`), so the
hygiene violations those waves introduced — bare F1/F2 "feature tags" in
five comments — sat green until the `windows` leg (which runs the FULL
`cargo test --workspace` under Wine) caught all five at close. The docs
`cargo xtask refs` step would have caught them per wave too — it was in
none of those runs.

**Rule:** the commit gate is the FULL form — `cargo xtask ci gui fmt
clippy test refs` (every leg named, de-duplicated, order-free). A bare
`ci gui` is only ever a fast re-check of the shell's clippy, never a
commit gate. The same substitution rule makes `cargo xtask ci windows`
the windows leg ALONE — fine as a named leg, misleading as a "quartet +
windows" shorthand.

Cross-ref: `gui-leg-lint-drift.md` (why the gui leg must ride viewer
waves), `refs-hygiene-prose-patterns.md` (the F-tag pattern itself).
