# oxidizr-cli

The user-facing binary: clap command surface, the miette diagnostic
renderer, and the outcome replay (stdout bytes, then the log). All
command logic lives in `oxidizr-api`; this crate only maps argv to a
`Request` and renders the `Outcome`.

Exit codes: `0` ok · `1` diagnostics / differences found · `2` usage ·
`3` I/O. `view` (GUI) and `completions`/`oracle` dispatch before the
pipeline.

The binary is `oxidizr`. See the root README for the command table.
