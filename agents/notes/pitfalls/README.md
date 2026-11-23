# pitfalls — traps already paid for

Verify before "fixing" something listed here — these were confirmed against the
tools, not guessed.

| note | one-liner |
|---|---|
| [rust-overlay-target-components.md](rust-overlay-target-components.md) | `rust-mingw` belongs in `targetExtensions`; a bad host `extensions` entry silently drifts the nightly ~100 nights |
| [cargo-bench-args.md](cargo-bench-args.md) | args after `cargo bench --` go to libtest harnesses, not criterion |
| [bench-machine-drift-ab.md](bench-machine-drift-ab.md) | baselines drift with machine state — same-session stash A/B (`--bench` target, log to file) proves the real delta |
| [nixpkgs-quirks.md](nixpkgs-quirks.md) | win32-threads mingw lacks winpthreads; nested lists deprecated; flake not nixfmt-styled |
| [expect-test-eol.md](expect-test-eol.md) | snapshots are byte-compared incl. EOL; `UPDATE_EXPECT` regenerates against the *local* checkout's EOL |
| [git-mv-deletes-untracked.md](git-mv-deletes-untracked.md) | `git mv dir/ …` removes untracked files inside `dir/` (they land in `~/.local/share/Trash`) |
| [fmt-line-end-guards.md](fmt-line-end-guards.md) | `split("\\r?\\n")` contract; `\`/`"""` line-end guards; joiner-only `stripLeading` |
| [reconcile-ambiguity-fixtures.md](reconcile-ambiguity-fixtures.md) | duplicate names die at parse/hydrate; contest needs identifier-vs-canonical on one new id |
| [dsl-corpus-grammar-gotchas.md](dsl-corpus-grammar-gotchas.md) | T2 e2e-corpus grammar traps: own-line comments only, identifier charsets, context-statement tables, hierarchical dotted scopes, extends-env via `!element` |
| [t3-invalid-corpus-probes.md](t3-invalid-corpus-probes.md) | T3 debt-code verdicts (3001/3002 reachable, 4002/9001 unreachable, 5001/5005 shapes), mutation-op rules, mutate-then-rebless |
| [shell-timing-and-cargo-clean.md](shell-timing-and-cargo-clean.md) | `/usr/bin/time` absent + SILENT in `2>` harnesses; `cargo clean --workspace` keeps deps warm; n≥3 for ms deltas |
| [dsl-style-color-normalization.md](dsl-style-color-normalization.md) | T4 oracle verdicts: `Color` setter normalization applies on the DSL path too (lowercase hex, name→hex, bogus = statement error); `group { url }` rejection is reference-correct |
| [epaint-037-font-stack.md](epaint-037-font-stack.md) | epaint 0.37: `default_fonts` OFF by default (silent empty font_data headless); skrifa not ab_glyph — outlines via companion ab_glyph; glyph/shape gotchas |
| [notify-windows-modify-any.md](notify-windows-modify-any.md) | notify 8 Windows backend maps every write to `Modify(Any)` (never `Modify(Data)`); cfg-widen pre-filters, hash layer decides; `#[cfg]` rejected in `matches!` pat |
| [agent-shell-quoting.md](agent-shell-quoting.md) | tool-call `\b` = JSON backspace; double-quoted backticks EXECUTE commands; subagent reports ≠ evidence (`git diff --stat` is) |
| [cargo-dist-installer-panic.md](cargo-dist-installer-panic.md) | dist 0.31–0.34 panics `couldn't find the release!?` whenever `installers` is set but GitHub hosting isn't; fix = v1 `dist-workspace.toml` with `ci`+`hosting` |
| [windows-path-separator-assertions.md](windows-path-separator-assertions.md) | test assertion hardcoded `/` met `Path::display`'s `\` on the first Windows run; assert the joined+rendered path; closes with filesystem tests need the windows leg |
