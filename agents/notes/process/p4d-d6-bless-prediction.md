# P4D D-6 bless prediction (V2, settled 2026-11-22)

Scan of every artifact that could move when D-6 rewrites human text:

- **e2e sidecars** (`e2e/corpus/**/workspace.e2e.json`, 307): fields are
  exit/codes/counts/files/idempotent ONLY — NO human text. **bless must NOT
  move**; a moving sidecar at D-6 = byte-contract red.
- **`testdata/golden/`**: workspace/diagram/theme bytes — no oxidizr message
  text. Must NOT move.
- **No help-snapshot goldens**: CLI tests probe `--help` loosely
  (`help.contains("--theme")`), never byte snapshots.
- **In-test message assertions update in place** (same commit as the text):
  `oxidizr-cli/tests/cli.rs` (~19 `stdout/stderr.contains(...)` + the T-6
  export note goldens), `oxidizr-api/tests/diff_probes.rs` (human ±
  report snapshots), `oxidizr-inspect/tests/*` (human `type | SEVERITY |
  message` lines), `oxidizr-lsp/tests/transcript.rs` (hover text),
  `oxidizr-cli/tests/cli.rs` did-you-mean strings.
- `docs/diagnostics.md` rows and `docs/*-schema.md` example messages are
  D-6 sources themselves (pass over them, no external golden).
