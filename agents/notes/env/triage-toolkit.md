# triage toolkit — inspection and search tools (what is on PATH)

For reading artifacts and searching code while debugging/verifying. All come
with `nix develop` unless marked (profile).

| tool | when | how |
|---|---|---|
| `rg` | text search (prefer the agent harness grep tool over raw rg) | `-F` for literals; line-based |
| `ast-grep` | structural code search / refactor surveys | recipes: testing/ast-grep-search.md |
| `jq` | peeking at golden/CLI JSON | `jq '.views[0]' out.json` — but the byte contract wins: diff bytes FIRST, jq for orientation |
| `sqlite3` | spelunking vendored DB fixtures | read-only: `sqlite3 -readonly f.db '.tables'` |
| `python3` | throwaway cross-checks (base64, Java-hash, UUID shape) | no package installs |
| `critcmp` | benchmark compare | rides `cargo xtask bench cmp <LABEL>` (toolbox.md) |
| `java` | poking the oracle war by hand | normal flow stays `cargo xtask oracle` |
| `gh` (profile) | reading issues / CI run logs | read-only: the charter says NEVER push |
| `dot` (graphviz), `plantuml` | P2/P3 render checks — NOT used in P1 | discovery via `$OXIDZR_PLANTUML_BIN` (flake comment) |

Env knobs: `NO_COLOR=1` → deterministic ASCII diagnostics (tests set it);
`PROPTEST_CASES=20000` for deep property runs (seeds land in
`*.proptest-regressions` — commit them); `UPDATE_EXPECT=1` for expect-test
snapshots (EOL caveat: pitfalls/expect-test-eol.md).
