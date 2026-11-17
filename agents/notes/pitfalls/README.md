# pitfalls — traps already paid for

Verify before "fixing" something listed here — these were confirmed against the
tools, not guessed.

| note | one-liner |
|---|---|
| [rust-overlay-target-components.md](rust-overlay-target-components.md) | `rust-mingw` belongs in `targetExtensions`; a bad host `extensions` entry silently drifts the nightly ~100 nights |
| [cargo-bench-args.md](cargo-bench-args.md) | args after `cargo bench --` go to libtest harnesses, not criterion |
| [nixpkgs-quirks.md](nixpkgs-quirks.md) | win32-threads mingw lacks winpthreads; nested lists deprecated; flake not nixfmt-styled |
| [expect-test-eol.md](expect-test-eol.md) | snapshots are byte-compared incl. EOL; `UPDATE_EXPECT` regenerates against the *local* checkout's EOL |
| [git-mv-deletes-untracked.md](git-mv-deletes-untracked.md) | `git mv dir/ …` removes untracked files inside `dir/` (they land in `~/.local/share/Trash`) |
| [fmt-line-end-guards.md](fmt-line-end-guards.md) | `split("\\r?\\n")` contract; `\`/`"""` line-end guards; joiner-only `stripLeading` |
| [reconcile-ambiguity-fixtures.md](reconcile-ambiguity-fixtures.md) | duplicate names die at parse/hydrate; contest needs identifier-vs-canonical on one new id |
| [dsl-corpus-grammar-gotchas.md](dsl-corpus-grammar-gotchas.md) | T2 e2e-corpus grammar traps: own-line comments only, identifier charsets, context-statement tables, hierarchical dotted scopes, extends-env via `!element` |
| [t3-invalid-corpus-probes.md](t3-invalid-corpus-probes.md) | T3 debt-code verdicts (3001/3002 reachable, 4002/9001 unreachable, 5001/5005 shapes), mutation-op rules, mutate-then-rebless |
