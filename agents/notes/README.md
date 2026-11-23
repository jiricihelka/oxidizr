# notes — index

Short, durable recall notes. One topic per note, ≤ 25 lines, no prose filler.
After fixing something non-obvious, add a note + one index row here and in the
subdir README (policy: [process/notes-policy.md](process/notes-policy.md)).

## env/ — dev environment setup ([index](env/README.md))

| note | one-liner |
|---|---|
| [wine-windows-leg.md](env/wine-windows-leg.md) | run Windows CI tests locally via windows-gnu + Wine |
| [reference-oracle.md](env/reference-oracle.md) | structurizr oracle: pins, invocation, speed |
| [nix-flake.md](env/nix-flake.md) | what the dev shell provides; nightly double-pin |
| [toolbox.md](env/toolbox.md) | gates + verification tools on PATH, when/how; what's absent |
| [triage-toolkit.md](env/triage-toolkit.md) | inspection/search tools + env knobs |
| [gui-runtime-nixos.md](env/gui-runtime-nixos.md) | viewer `NoWaylandLib` on NixOS — dev-shell `LD_LIBRARY_PATH` feeds winit's runtime dlopens |

## pitfalls/ — traps already paid for ([index](pitfalls/README.md))

| note | one-liner |
|---|---|
| [rust-overlay-target-components.md](pitfalls/rust-overlay-target-components.md) | `rust-mingw` needs `targetExtensions`, else nightly silently drifts |
| [cargo-bench-args.md](pitfalls/cargo-bench-args.md) | `cargo bench -- --flag` breaks libtest harnesses |
| [bench-machine-drift-ab.md](pitfalls/bench-machine-drift-ab.md) | bench baselines drift with machine state — same-session stash A/B proves the real delta |
| [nixpkgs-quirks.md](pitfalls/nixpkgs-quirks.md) | win32-mingw has no winpthreads; `wine` not `wine64`; misc |
| [expect-test-eol.md](pitfalls/expect-test-eol.md) | snapshots pin bytes incl. EOL; UPDATE_EXPECT pitfalls |
| [git-mv-deletes-untracked.md](pitfalls/git-mv-deletes-untracked.md) | dir moves eat untracked files; Trash first |
| [agent-shell-quoting.md](pitfalls/agent-shell-quoting.md) | tool-call `\b` = JSON backspace; double-quoted backticks EXECUTE; subagent reports ≠ evidence (`git diff --stat` is) |
| [fmt-line-end-guards.md](pitfalls/fmt-line-end-guards.md) | fmt: CR-split contract, `\`/`"""` end guards, joiner-only stripLeading |
| [reconcile-ambiguity-fixtures.md](pitfalls/reconcile-ambiguity-fixtures.md) | duplicate names die at parse/hydrate; contest needs identifier-vs-canonical |
| [dsl-corpus-grammar-gotchas.md](pitfalls/dsl-corpus-grammar-gotchas.md) | e2e-corpus grammar traps: own-line comments, identifier charsets, context-statement tables, dotted scopes, extends-env via `!element` |
| [shell-timing-and-cargo-clean.md](pitfalls/shell-timing-and-cargo-clean.md) | `/usr/bin/time` absent silently; `cargo clean --workspace` keeps deps warm; n≥3 timings |
| [epaint-037-font-stack.md](pitfalls/epaint-037-font-stack.md) | epaint 0.37: `default_fonts` OFF by default; skrifa-not-ab_glyph ⇒ companion-dep glyph outlines |
| [notify-windows-modify-any.md](pitfalls/notify-windows-modify-any.md) | notify 8 Windows backend: plain writes are `Modify(Any)` never `Modify(Data)` ⇒ reload edges dead on Windows CI; hash layer is the contract; `#[cfg]` illegal in `matches!` patterns |

## testing/ — test architecture ([index](testing/README.md))

| note | one-liner |
|---|---|
| [byte-exact-corpus.md](testing/byte-exact-corpus.md) | testdata/ is a byte contract; .gitattributes EOL pinning |
| [burn-in-register.md](testing/burn-in-register.md) | m4_fixtures KNOWN_ERRORS pattern (both-directions check) |
| [local-ci-mirrors.md](testing/local-ci-mirrors.md) | `cargo xtask ci\|bench\|cov` mirrors ci.yml argv |
| [ast-grep-search.md](testing/ast-grep-search.md) | `ast-grep` structural search recipes, vs grep counts |
| [oracle-app-harness.md](testing/oracle-app-harness.md) | export injects inspection props + default views; goldening limits |
| [cli-e2e-pitfalls.md](testing/cli-e2e-pitfalls.md) | subprocess tests: pipe stdio, random UUIDs, known-vs-legacy JSON fields |
| [fuzz-p4t-finds.md](testing/fuzz-p4t-finds.md) | fuzz battery finds (fmt BOM, zero-width matcher doubling) + bounded-surface rule |

## perf/ — measured timings ([index](perf/README.md))

| note | one-liner |
|---|---|
| [scene-paint-budgets.md](perf/scene-paint-budgets.md) | P3-M5: scene rebuild + paint vs the 60 fps / 200-element budget (≈1 ms/frame ⇒ ~16× headroom) |

## process/ — how work is done ([index](process/README.md))

| note | one-liner |
|---|---|
| [commit-discipline.md](process/commit-discipline.md) | gate trio; commit EVERY green sub-piece; never push |
| [refactor-patterns.md](process/refactor-patterns.md) | P2R's zero-behavior-change playbook: phantom-Result cascade, push-order, rng-draw dedupe, bless-as-net |
| [resume-ritual.md](process/resume-ritual.md) | rewrite `RESUME.md` at each green commit; resume from it |
| [faithful-port.md](process/faithful-port.md) | engine = port of pinned Java, lint allows need rationale |
| [notes-policy.md](process/notes-policy.md) | when and how to write notes |
| [parallelism.md](process/parallelism.md) | one-LLM box: background jobs, no subagents (9 concurrent = server memory thrash; cap 3) |
| [p4d-strip-rules.md](process/p4d-strip-rules.md) | P4D comment-strip pass contract (comments only, facts restated, verify recipe) |
| [p4d-d6-bless-prediction.md](process/p4d-d6-bless-prediction.md) | what may/must not move when D-6 rewrites human text (bless stays 307×) |
