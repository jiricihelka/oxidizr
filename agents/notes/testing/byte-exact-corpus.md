# testdata/ is a byte contract

Goldens are byte-compared; vendored fixtures must keep reference bytes. Two EOL
layers must both hold on every checkout:

1. **Repo text**: root `.gitattributes` `* text=auto eol=lf` — sources/snapshots
   check out LF (beats `core.autocrlf=true`). Without it, `expect!` CST snapshots
   fail on Windows (see pitfalls/expect-test-eol.md).
2. **Corpus**: `testdata/** -text` — no conversion in either direction, so fixture
   *content* (what the lexer snapshots) equals the vendored bytes; re-vendoring
   from the reference stays byte-true.

CI never regenerates goldens (committed; oracle is Category B, weekly). When
vendoring new fixtures record the tool version in testdata/README.md.
macOS-specific fs behavior is CI-only — Wine/Linux can't approximate it.

Last useful: never
