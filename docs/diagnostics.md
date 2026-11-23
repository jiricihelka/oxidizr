# Diagnostics registry (`OXZ` codes)

The stable diagnostic codes emitted by oxidizr. Codes are stable after
1.0; retiring a code leaves a tombstone row here and the number is never
reused. Every code the codebase can emit is listed; the emitter lives in
`crates/oxidizr-diag`.

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
| `OXZ-1001` | Error | unterminated quoted string at end of line | Reserved, never emitted by default: an unterminated quote does not fail lexing (the official tooling does not error here either); kept for future linting. |
| `OXZ-2001` | Error | `Unexpected tokens (expected: …)` | Token outside the set permitted at this point in the grammar; the message lists the permitted set and a did-you-mean when one is near. |
| `OXZ-2002` | Error | reference statement message | A statement rejected by the grammar (unknown identifier, duplicate name, bad placement, …) — the message is the official tooling's, verbatim. |
| `OXZ-2003` | Error | `Unexpected end of DSL content - are one or more closing curly braces missing?` | An unclosed block at EOF. Add the missing closing braces. |
| `OXZ-3001` | Error | reference validation message | A model/view validation rule was violated (`oxidizr validate`); the message is the official tooling's, verbatim. |
| `OXZ-3002` | Error | `ambiguous reconcile: …` | A layout id matches more than one re-parsed id (or two match one) — never guessed, the entry is not remapped. |
| `OXZ-4001` | Error | `failed to parse JSON: …` / `failed to read workspace: …` / `workspace validation failed: …` | JSON input rejected: syntax, structural read, or validation. Fix the input file. |
| `OXZ-4002` | Error | `failed to write workspace: …` | The model cannot be written byte-faithfully (e.g. a filtered view without a base view). |
| `OXZ-5001` | Error | network disabled | A theme URL was referenced without `--allow-net`. Pass `--allow-net` or use an installed theme; URL inputs `!include <url>` / `workspace extends <url>` are always rejected. |
| `OXZ-5002` | Error | JVM-only directive not supported | `!script`, `!plugin`, `!components`, `!impliedRelationships <fqcn>` need the JVM import stack and are unsupported. |
| `OXZ-5003` | — | (tombstone) | Retired when the `!docs`/`!adrs` importers landed; never emitted, must not be reused. |
| `OXZ-5004` | — | (tombstone) | Retired when `workspace extends` landed; never emitted, must not be reused. |
| `OXZ-5005` | Error | server-side import deferred | A `theme <file>` content load or a raster/plantuml/mermaid/kroki `image` import was attempted; these need the official import stack and are unsupported. Raster `image <file>` in DSL views is `OXZ-5002`. |
| `OXZ-5006` | Error | `cannot read/write …` | CLI I/O and input policy: unreadable/unwritable file, stdio error, non-UTF-8 input, undetectable workspace format. Exit code 3. |
| `OXZ-5007` | Error | export format not implemented | `export --format` named a registered format whose exporter has not landed (`static`/`png`/`svg`). Truly-unknown format strings are NOT a diagnostic — they log `Unknown export format: …` and exit 0, like the official CLI. |
| `OXZ-5008` | Error | theme loading failed | An installed theme's `theme.json` is unreadable/unparsable, or a fetched theme's content-type is neither JSON nor plain text, or the fetch failed (including no system `curl` under `--allow-net`). A theme URL without `--allow-net` is `OXZ-5001`. |
| `OXZ-5009` | Error | export aborted | The diagram engine failed fatally mid-export (a view-kind cast mismatch, a dangling id dereferenced while writing); reported instead of panicking. |
| `OXZ-5010` | Error | `generate` failed | An `--include`/`--exclude` pattern uses a construct outside the supported full-match filter subset (`{m,n}` quantifiers, lookaround, `\p{…}`, named groups, inline flags), or no workspace matched. Narrow the pattern; it is never silently widened or narrowed. |
| `OXZ-9001` | Error | too many errors | Diagnostic collection aborted at the 100-entry cap; further details are replaced by one summary note. |
