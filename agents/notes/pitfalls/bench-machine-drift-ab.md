# Bench baselines drift with machine state — A/B the delta, don't trust `cmp`

`cargo xtask bench cmp <LABEL>` compares against a baseline captured on
another day. E-3 (2026-11-22) hit the classic false alarm: the full suite
came back 1.03–1.32× slower **in groups no commit touched** (e.g.
`m7_dsl_parse_corpus` 1.32×) — the machine was simply slower than when
`P3` was saved (untouched-code legs measured 1.11–1.28× against their own
P3-era numbers). Reading `cmp` as a verdict would have blamed the lane
pass for the whole suite.

Protocol that settles it (same-session A/B of the two code states):

```sh
git stash push -m ab -- <the-one-changed-file>
cargo bench -q -p <crate> --bench <target> -- --save-baseline ab-pre
git stash pop
cargo bench -q -p <crate> --bench <target> -- --save-baseline ab-post
critcmp ab-pre ab-post
```

Rules learned:

- `--bench <target>` ONLY targets the criterion harness — bare `-p <crate>`
  forwards the flags into the libtest bench harness too and dies (same
  trap class as `pitfalls/cargo-bench-args.md`).
- Pipe bench output to a FILE, not `grep | head` — SIGPIPE from `head`
  kills the run mid-suite and the baseline saves half.
- Alternate legs (pre, post, pre2, post2) when a group is close: the
  E-3 call was made on two legs each (`mixed` +13% → +8% after a
  BTreeMap→flat-vec+sort rewrite; `chain`/`star` neutral).
- `critcmp` baselines live under `target/criterion` (scratch, untracked) —
  extra `ab-*` ids are harmless; the committed gate stays `bench cmp <LABEL>`.

Last useful: E-3 stamp in `agents/plan/build/PHASE_4.md` §P4-M5.
