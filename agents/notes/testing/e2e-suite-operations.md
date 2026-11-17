# e2e suite operations (P1T close-out)

Last useful: 2026-11-17 · `crates/oxidizr-e2e/`, plan `build/PHASE_1T.md`.

- Tiers: quick = sidecars ride `cargo test --workspace` (every PR); full =
  `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e --test full` (nightly
  `e2e.yml`; env gate, NOT `#[ignore]` — V-T4).
- Corpus tools: `cargo xtask e2e bless` re-records sidecars (review the diff!);
  `cargo xtask e2e mutate` regenerates `corpus/mutated/` WHOLESALE — always
  re-bless after (details: `pitfalls/t3-invalid-corpus-probes.md`).
- **Bless-review rule** (P1T risk register): sidecars assert USE-CASE shape —
  exit class, OXZ code set, counts — never bytes or message text. A legit
  refactor blesses with tiny/no diffs. The smell a reviewer must catch: a
  bless diff that changes the EXIT CLASS or the COUNTS of fixtures unrelated
  to the change under review — that is an engine behavior change wearing a
  test-update costume; stop and get an oracle verdict before blessing
  (`env/reference-oracle.md`).
- Adding entries: valid entries need only `manifest.toml` + `workspace.dsl`
  (+ children for `!include`/`extends`) then bless; invalid entries must carry
  ≥1 OXZ code (the coverage gate counts them; allow-list {1001, 4002, 9001}).
- Benches: feature `e2e-bench` (`required-features`) — plain
  `cargo bench --workspace` skips them by design (local-only, V-T5);
  `cargo xtask bench` opts in, `bench save <LABEL>`/`cmp` as usual.
