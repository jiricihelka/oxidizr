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

## pitfalls/ — traps already paid for ([index](pitfalls/README.md))

| note | one-liner |
|---|---|
| [rust-overlay-target-components.md](pitfalls/rust-overlay-target-components.md) | `rust-mingw` needs `targetExtensions`, else nightly silently drifts |
| [cargo-bench-args.md](pitfalls/cargo-bench-args.md) | `cargo bench -- --flag` breaks libtest harnesses |
| [nixpkgs-quirks.md](pitfalls/nixpkgs-quirks.md) | win32-mingw has no winpthreads; `wine` not `wine64`; misc |
| [expect-test-eol.md](pitfalls/expect-test-eol.md) | snapshots pin bytes incl. EOL; UPDATE_EXPECT pitfalls |
| [git-mv-deletes-untracked.md](pitfalls/git-mv-deletes-untracked.md) | dir moves eat untracked files; Trash first |
| [fmt-line-end-guards.md](pitfalls/fmt-line-end-guards.md) | fmt: CR-split contract, `\`/`"""` end guards, joiner-only stripLeading |
| [reconcile-ambiguity-fixtures.md](pitfalls/reconcile-ambiguity-fixtures.md) | duplicate names die at parse/hydrate; contest needs identifier-vs-canonical |
| [dsl-corpus-grammar-gotchas.md](pitfalls/dsl-corpus-grammar-gotchas.md) | e2e-corpus grammar traps: own-line comments, identifier charsets, context-statement tables, dotted scopes, extends-env via `!element` |

## testing/ — test architecture ([index](testing/README.md))

| note | one-liner |
|---|---|
| [byte-exact-corpus.md](testing/byte-exact-corpus.md) | testdata/ is a byte contract; .gitattributes EOL pinning |
| [burn-in-register.md](testing/burn-in-register.md) | m4_fixtures KNOWN_ERRORS pattern (both-directions check) |
| [local-ci-mirrors.md](testing/local-ci-mirrors.md) | `cargo xtask ci\|bench\|cov` mirrors ci.yml argv |
| [ast-grep-search.md](testing/ast-grep-search.md) | `ast-grep` structural search recipes, vs grep counts |
| [oracle-app-harness.md](testing/oracle-app-harness.md) | export injects inspection props + default views; goldening limits |
| [cli-e2e-pitfalls.md](testing/cli-e2e-pitfalls.md) | subprocess tests: pipe stdio, random UUIDs, known-vs-legacy JSON fields |

## process/ — how work is done ([index](process/README.md))

| note | one-liner |
|---|---|
| [commit-discipline.md](process/commit-discipline.md) | gate trio; commit EVERY green sub-piece; never push |
| [resume-ritual.md](process/resume-ritual.md) | rewrite `RESUME.md` at each green commit; resume from it |
| [faithful-port.md](process/faithful-port.md) | engine = port of pinned Java, lint allows need rationale |
| [notes-policy.md](process/notes-policy.md) | when and how to write notes |
| [parallelism.md](process/parallelism.md) | one-LLM box: background jobs, no subagents |
