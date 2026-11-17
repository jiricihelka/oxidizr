# T3 invalid-corpus probes (debt codes + mutation ops)

Why: reaching the last five registry codes and making `xtask e2e mutate`
honest took empirical probes; the verdicts would otherwise be re-derived at
the next corpus change. Full text: `plan/build/PHASE_1T.md` §P1T-D4 landed.

Facts (all probed against `target/debug/oxidizr`, NO_COLOR+ANSI-strip):

- 3001 (VALIDATION) needs a PARSE-CLEAN workspace: person + softwareSystem
  sharing a name passes parse (kind-prefixed ids) and fails `validate`.
  Grammar errors always beat validation — no grammar break can cover 3001.
- 3002 (RECONCILE) needs the `reconcile` command: corpus entries with BOTH
  `workspace.dsl` + `workspace.json` now run `Request::Reconcile` (manifest
  section `reconcile`). Fixture shape: `pitfalls/reconcile-ambiguity-
  fixtures.md` (identifier-vs-canonical claim on one new id).
- 4002 (JSON_WRITE) is UNREACHABLE through the corpus: every shape the writer
  refuses (filtered view w/o base, dangling view links) is rejected by the
  JSON hydrate gate at READ (4001 first); `from_slice` validation covers
  filtered-base, dup view keys, wrong-kind view links. Allow-listed.
- 5001 (NET_DISABLED) comes off the old "needs --allow-net" reasoning:
  `!include https://…` → clean [5001]; `workspace extends <url>` →
  [2002,5001]. Policy errors surface as coded `DslError`, no flag involved.
- 5005 (IMPORT_DEFERRED): `theme <existing-file>` → clean [5005]; missing
  file → 2002; `theme <url>` parses CLEAN (url themes are data until render).
  `plantuml <file>`/`plantuml <viewKey>` → 2001 (grammar rejects before the
  deferred importer).
- 9001 (TOO_MANY_ERRORS) is UNWIRED: `oxidizr_diag::Collector` (CAP=100 +
  overflow note via `finish()`) is referenced nowhere in api/cli/dsl engine —
  120 junk lines emit 120 errors, no note. Allow-listed until the wiring
  decision lands.
- `!plugin "x"` → clean 5002; `!adrs <dir>`/`!docs <dir>` → clean 5003
  (directory must exist); dup container / dup deploymentNode names →
  parse-time 2002; unterminated quote → lexer runs to EOL, file may still
  parse CLEAN (1001 stays allow-listed).
- Mutation op `break ${` is broken-by-design: hermetic empty env passes raw
  placeholders through ⇒ still-valid workspace (a mutant proved it). Ops must
  be verified to FAIL — `mutate::write_corpus` runs dsl2json per mutant and
  errors the tool if one parses clean.
- `mutate` REPLACES `corpus/mutated/` wholesale (sidecars included): always
  `cargo xtask e2e bless mutated` after a mutate run.

Testing: `cargo test -p oxidizr-e2e` (coverage gate asserts the allow-list
shrunk to {1001, 4002, 9001}; a covered-but-listed code fails the test).
