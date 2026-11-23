# P4D strip rules (rustdoc/comment pass)

Digest of `agents/plan/build/PHASE_4D.md` §P4D-D1 for per-crate strip passes.

1. Touch ONLY comments (`//!` `///` `//`). NEVER string literals, code,
   identifiers or macro contents — not one output byte may change (D-6 owns
   string text).
2. Delete: `§P…`, `P1-Dn`…`P4-Dn`, `F1`…`F12`, `Pn-Mn`, `(Dn)`, `(V-n)`,
   `(W-n)`, "Phase N", "milestone", "wave", "interlude", commit hashes,
   pins (`ad34036`), dates, "ported from …", Java/monorepo provenance
   narration, `agents/plan/…` path citations in doc comments.
3. No reference-implementation narration ("the reference does X", Jackson,
   ObjectMapper, Java class/method names like `hydrate()`). Restate the
   FACT plainly. Compat facts stay: "byte-for-byte identical to what the
   official Structurizr tooling writes", "the official tooling's message,
   verbatim". Crash-parity traps: "the official tooling crashes here;
   oxidizr writes `""` instead (a documented deviation)".
4. Keep every engineering fact: invariants, traps, ordering contracts,
   bug-compat behavior — short declarative sentences. Restate, never
   invent.
5. Keep rustdoc links valid; `cargo doc -p <crate> --no-deps` must stay
   warning-free.
6. "reference" in its technical sense (a pointer/`&`) STAYS; only the
   official-implementation sense is stripped (say "official tooling").
7. `cfg(test)` module comments: same rules.
8. ONE pass per crate — style-only re-diffs of already-clean text are
   rejected.
9. Verify: `cargo fmt -p <crate>` + `cargo test -q -p <crate>` green;
   `grep -rn '§P\|P[1-4]-D\|ad34036' crates/<crate>/src --include=*.rs`
   empty; residual `grep -rni 'reference'` hits all technical-use.
10. Do NOT commit; do NOT touch other crates or `cargo xtask` (wave end).
11. Traps learned in wave A: a global whitespace-collapse (`  +` → ` `)
    crushes doc-list continuation indents → clippy `doc_markdown` storm;
    bullet continuations take exactly 3 spaces (numbered lists flaky —
    convert to bullets). CamelCase doc words (`OpenAPI`, `PlantUML`) need
    backticks; re-check backtick pairs after any phrase-regex pass. Run
    `cargo clippy --workspace --all-targets` before the wave commit.
12. Comment-only audit: `git diff -U0`, drop lines whose content starts
    `//`, strip trailing `//` comments, compare the code sides pairwise —
    zero mismatches proves no code byte moved.
