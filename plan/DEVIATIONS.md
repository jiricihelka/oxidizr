# DEVIATIONS.md — intentional deviations from Structurizr

Behavioral differences between oxidizr and the reference (Java) implementation, agreed
here BEFORE implementation. Purely additive features (things Structurizr does not have
at all) live in `ADDITIONAL_FEATURES.md`; this file only covers points where the
reference HAS a behavior and we deliberately differ.

The compatibility contract is absolute:

> **Contract**: for any input accepted by the reference, oxidizr must produce the same
> in-memory workspace and the same JSON output (byte-identical: alphabetical keys,
> NON_EMPTY omission, UTC-Z dates, 2-space indent, string ids). Deviations are allowed
> ONLY where the contract is silent: diagnostics/stderr/exit codes, CLI ergonomics, new
> commands, and behavior behind explicit opt-in flags. Anything else is a bug.

Reference pinned: monorepo `ad34036` (v6.2.4). Details in `plan/`.
Ground rule for future changes: a new deviation needs (a) an entry below, (b) proof the
default output still matches the reference goldens.

## D1 — Rich diagnostics instead of first-error (default)

- Structurizr: parse aborts at the first error; `<msg> at line N of <file>: <line text>`,
  no column; unknown statement → `"X is not valid here"`; deprecated tokens fail mid-parse.
- oxidizr: all parse + validation errors collected and reported together, with
  line+column+snippet, "did you mean" suggestions for misspelled keywords, and warnings
  for deprecated-but-accepted tokens (`!ref`, `metadata`-as-`tags`, legacy JSON fields).
  `--strict` promotes warnings to errors.
- Compatibility impact: none (stderr/exit codes only). Exit codes must stay CI-sane:
  0 ok / 1 error / 2 usage.

## D2 — JVM-only directives are rejected cleanly

- Structurizr: `!script` (JSR-223), `!plugin` (JAR SPI), `!components` (bytecode/source
  analysis), `!impliedRelationships <fqcn>` execute arbitrary Java/JVM code.
- oxidizr: these tokens are recognized and rejected with a precise, actionable error
  (construct, file/line, why, suggested portable alternative — e.g. restructure with
  `!include` / `workspace extends`, declare components manually). Workspaces using them
  are, by definition, out of supported scope.
- Explicitly REJECTED alternative (do not revisit without new evidence): emulating
  `!script` with an embedded JS engine — Structurizr scripts target Nashorn/GraalJS and
  the Java API, so "same input, different workspace" is likely; silent divergence is
  worse than honest rejection.
- Roadmap (undated): out-of-process extension protocol (exec + JSON-RPC) as a native
  plugin story; must never change parse results for JVM-plugin-free workspaces.

## D3 — JSON round-trip: reference-exact by default, lossless opt-in

- Structurizr: reads unknown/legacy fields leniently, then silently DROPS them on write
  (e.g. `location`, `branding`, `enterprise` from v5 workspaces).
- oxidizr: default = exact reference parity (drops them too). `--keep-unknown` (off by
  default) preserves unknown AND known-legacy fields verbatim on JSON→JSON round-trips
  for migration tooling — "superset-preserving round-trip" mode. Output remains valid
  Structurizr JSON either way (readers ignore unknowns).

## D4 — (moved) Additive commands

New first-class commands are not deviations; they are registered in
`ADDITIONAL_FEATURES.md` (F1 `reconcile`, F2 `fmt`, F3 `diff`, F4 `oracle`). D4 is kept
as a tombstone so old references keep resolving.

## D5 — No implicit network access (default)

- Structurizr: fetches themes from `https://static.structurizr.com/...` during load/
  export; `!include <url>` fetches URLs; offline support is an env-var convention.
- oxidizr: never touches the network unless explicitly asked. Default theme (and any
  referenced theme URL needed for diagram export) resolves from a local cache /
  `--themes-dir` (same layout as `STRUCTURIZR_THEMES`); `!include <url>` requires
  `--allow-net`. JSON output is unaffected in all cases (theme URLs are stored verbatim;
  resolution only ever matters for diagram rendering/export).

## D6 — Internal quality decisions (no behavioral surface)

- Immutable domain model; no exceptions mid-construction; validation aggregated at
  defined checkpoints (vs Java setters throwing `IllegalArgumentException`, producing
  partially built workspaces). Validation semantics themselves match the reference
  (rule catalogue: `plan/FIXTURES.md` §C validation fixtures).
- Schema-first: the JSON contract is generated from one machine-readable definition and
  drift-checked against goldens in CI — preventing Structurizr's chronic
  schema-lags-code failures (schema 6.1.0 vs code 6.2.4 at time of writing).
- Diagnostics-grade lexer/parser with a real CST feeding workspace construction, enabling
  D1, `fmt`, `diff` (vs Java's per-scope keyword dispatch over preprocessed strings).

## Locked — must NEVER deviate (for contributors and future agents)

Shared sequential integer id counter (elements+relationships, definition order) · exact
implicit tag stacks & order, tagless implied relationships · `${VAR}` pre-pass semantics
incl. env fallback behind the same feature default · DSL portability rules and
`structurizr.dsl`/`structurizr.dsl.identifier` retention · auto view keys `Prefix-NNN` ·
environment flattening (`environment` string per node) · JSON details: alphabetical keys,
NON_EMPTY omission, `color` key, comma-string tags, string ids / numeric workspace id,
UTC-Z dates, 2-space indent · lenient JSON reading (unknowns ignored) · default views
generation only where the reference does it (app layer), never in parser/library.

Anything contradicting this list is fixed by changing oxidizr, per AGENTS.md.
