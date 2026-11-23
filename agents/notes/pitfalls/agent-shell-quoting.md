# agent shell/tool-call quoting traps

Mechanical traps in how command strings reach bash from the agent harness.
Each one already cost a paid debugging loop.

- **`\b` in a tool-call command string is a JSON backspace (0x08)**, not a
  regex boundary: `grep 'foo\b'` arrives mangled. Write boundary matches as
  `grep -w` / `-E '[^a-z]foo[^a-z]'`, or skip sed entirely and use the edit
  tool with `replaceAll: true` for renames.
- **Backticks inside double-quoted shell strings execute** (command
  substitution): a `grep "... \`oxidizr\` ..."` ran the `oxidizr` binary
  instead of matching it. Single-quote every grep pattern that can contain
  backticks or `$`; verify a template-scan actually read files, not ran the
  project.
- **Subagent reports are not evidence** — the first one claimed a toolchain
  failure and had edited nothing; cancelled ones leave partial edits behind.
  The only truth is `git diff --stat` + the comment/code audit + crate gates
  (see process/parallelism.md — subagents default: never).

Last useful: 2026-11-22
