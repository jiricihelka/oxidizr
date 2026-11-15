# expect-test snapshots are byte-exact (EOL!)

`expect_test` compares the *bytes* of the snapshot against actual output, and the
snapshot lives in a .rs source file — so checkout-side EOL conversion breaks
Windows legs while the code is fine. Seen on windows-latest (M4): `\n` snapshots vs
CRLF working tree (core.autocrlf=true) ⇒ `lexer::tests::snapshot_*` failed; the
`--include-stdout` failures were CRLF inside fixture *content* (see
byte-exact-corpus.md).

Fixes in place: root `.gitattributes` (`* text=auto eol=lf`, `testdata/** -text`,
`reports/** -text`) pins LF for sources/goldens repo-wide.

Do NOT "fix" a red Windows snapshot by running `UPDATE_EXPECT=1` locally on Linux —
that rewrites the snapshot to your platform bytes and just moves the mismatch.
`include_str!` fixtures always read the repo bytes — those tests are EOL-immune by
design (m4_fixtures pattern).
