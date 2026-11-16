# Diagnostics registry (`OXZ` codes)

Target document for `oxidizr-diag`'s stable diagnostic codes (P1-D2). Codes are stable
after 1.0; retiring a code leaves a tombstone row here. Range owner: `crates/oxidizr-diag`.

## Ranges

| Range | Area |
|---|---|
| `OXZ-1xxx` | lexer |
| `OXZ-2xxx` | parser / grammar |
| `OXZ-3xxx` | model & validation |
| `OXZ-4xxx` | JSON |
| `OXZ-5xxx` | I/O & CLI policy |
| `OXZ-9xxx` | internal |

## Registered codes

| Code | Severity | Message shape | Notes |
|---|---|---|---|
| `OXZ-2001` | Error | `Unexpected tokens (expected: …)` | token rejected by the context's permitted set / unexpected identifier, inside the statement engine |
| `OXZ-2002` | Error | reference statement message | any statement rejected by the faithful reference port (grammar, unknown identifier, duplicate name, …) — the reference text travels verbatim |
| `OXZ-2003` | Error | `Unexpected end of DSL content - are one or more closing curly braces missing?` | unclosed context at EOF |
| `OXZ-5001` | Error | network disabled | `!include <url>` / `workspace extends <url>` / theme URL fetch without `--allow-net` (D5; flag ships P2) |
| `OXZ-5002` | Error | JVM-only directive not supported | `!script`, `!plugin`, `!components`, `!impliedRelationships <fqcn>` (D2) |
| `OXZ-5003` | Error | documentation import arrives in Phase 4 | `!docs`/`!adrs` statement parsed, importer deferred (D7, retired by P4-D13) |
| `OXZ-5004` | — | (tombstone) | retired at M5 (D9): `workspace extends` landed with file and JSON bases; never emitted, must not be reused |
| `OXZ-5005` | Error | import arrives in Phase 4 | `plantuml`/`mermaid`/`kroki` render branches and `theme <file>` content load parsed, importer deferred (D11, retired by P4); raster `image <file>` stays OXZ-5002 (D10) |
