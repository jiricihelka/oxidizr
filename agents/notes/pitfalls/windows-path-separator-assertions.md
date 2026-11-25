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

## Addendum (v0.5.0 close, Wine sweep)

Three more shapes of the same trap, found running `cargo test --workspace
--target x86_64-pc-windows-gnu` under Wine:

* **Canonicalize is verbatim under Wine** (`\\?\Z:\…`) while the product's
  lexical `java_absolute` (the `getAbsolutePath` port) drops the prefix and
  root-renders `/`. Guard-message expectations must run the scratch dir
  through the SAME lexical function (`p4m2_docs_adrs.rs` has a test-local
  copy), never through the raw canonicalize output.
* **Port the reference's `File.separator`, don't hardcode `/`** — the `!docs`
  filename trim (`startsWith(File.separator)` → substring(1)) left `\name`
  prefixes on Windows targets; the port fixed to
  `starts_with(std::path::is_separator)` (Linux bytes unchanged, bless-verified).
  Where the REFERENCE is platform-aware, the product must be — assert the
  bare name, do not paper over with `file_name()`.
* **Wine runs find platform-INDEPENDENT bugs** — a random proptest seed hit
  an fmt continuation-join edge; the failure was written into the tracked
  `.proptest-regressions` file and then replayed RED on Linux. That replay
  is proof, not noise: fix the product, never delete the regression line.
