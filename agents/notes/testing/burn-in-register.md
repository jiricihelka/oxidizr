# m4_fixtures KNOWN_ERRORS register (burn-in pattern)

`crates/oxidizr-dsl/tests/m4_fixtures.rs` parses every `testdata/dsl/**/*.dsl`
with file-based `ParseOptions`. Each file must parse clean OR be registered as
`(relpath, OXZ code, message prefix)` in `KNOWN_ERRORS`.

The register is checked **both directions**: unregistered failures fail the gate,
AND stale registrations (fixture green now, or code/message changed) fail too —
so deferrals get pruned in lockstep with the milestone retiring them.

Tags: `M5` = `workspace extends` (OXZ-5004); `D7/P4` = `!docs`/`!adrs` importers
(OXZ-5003); `D11/P4` = image imports (OXZ-5005); permanent by design: `D2`
(JVM-only directives), `D5` (no network), `D10` (no JVM image codecs).
`component:`-tagged files are include/extends *children* — they legitimately fail
standalone with the reference "expected: workspace" error; covered via entries.

When landing a deferred feature: delete its register rows in the same commit.

Last useful: never
