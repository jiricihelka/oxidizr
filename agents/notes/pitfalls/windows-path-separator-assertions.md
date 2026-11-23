# Test assertions must not hardcode path separators

**Symptom.** The FIRST native-Windows run of `cargo test --workspace`
(CI `windows-latest`, first push after Phase 4) failed one test:
`generate_probes::broken_workspace_is_skipped_with_note` —
`skip note names the file: "Could not parse C:\Users\RUNNER~1\...\gamma\workspace.dsl: ..."`.

**Cause.** The assertion compared the skip note against
`contains("gamma/workspace.dsl")` with a hardcoded `/`, while the product
names the file via `Path::display` — which renders `\` on Windows targets.
The product was correct; only the assertion carried the separator
assumption. Two other path forms in the same note are normal Windows
behaviour, not bugs: the parser half shows `\\?\C:\...` (canonicalize's
verbatim prefix) while the file-name half shows the 8.3-shortened
`RUNNER~1` TEMP.

**Fix.** Assert against the joined-and-rendered path —
`let p = dir.join("x"); note.contains(&p.display().to_string())` — which is
platform-neutral. Sweep: grep `contains("[a-z0-9_.-]*/` under `crates/*/tests`.

**Process lesson.** CI's windows leg runs the FULL workspace test suite; a
close whose diff adds filesystem-touching tests must also run
`cargo xtask ci windows` (Wine). This test shipped unrun on any Windows
target from its milestone to the release.
