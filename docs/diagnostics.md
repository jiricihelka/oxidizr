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
| `OXZ-2001` | Error | `"<X>" is not valid here` | unknown token in scope context + "did you mean" from permitted set (reserved, lands with M4) |
| `OXZ-5001` | Error | network disabled | `!include <url>` / `workspace extends <url>` / theme URL fetch without `--allow-net` (D5; flag ships P2) |
| `OXZ-5002` | Error | JVM-only directive not supported | `!script`, `!plugin`, `!components`, `!impliedRelationships <fqcn>` (D2) |
| `OXZ-5003` | Error | documentation import arrives in Phase 4 | `!docs`/`!adrs` statement parsed, importer deferred (D7, retired by P4-D13) |
