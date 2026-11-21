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
| `OXZ-3001` | Error | reference validation message | `oxidizr validate` (M6): model/view rule from the M2 catalog violated — reference `hydrate()` text verbatim |
| `OXZ-3002` | Error | `ambiguous reconcile: …` | `oxidizr reconcile` (M6-c): a layout id matches >1 re-parsed id (or two match one) — never guessed, entry not remapped (F1, P1-D8) |
| `OXZ-4001` | Error | `failed to parse JSON: …` / `failed to read workspace: …` / `workspace validation failed: …` | JSON input rejected — syntax, structural read or hydrate validation (`JsonError::Parse/Read/Validation`) |
| `OXZ-4002` | Error | `failed to write workspace: …` | the model cannot be written byte-faithfully (`JsonError::Write`, e.g. filtered view without base) |
| `OXZ-5001` | Error | network disabled | theme URL fetch without `--allow-net` (D5, flag ships P2-M1; `!include <url>` / `workspace extends <url>` are rejected regardless of the flag — hardened P1 behavior) |
| `OXZ-5002` | Error | JVM-only directive not supported | `!script`, `!plugin`, `!components`, `!impliedRelationships <fqcn>` (D2) |
| `OXZ-5003` | — | (tombstone) | retired at P4-M2 (D7 by P4-D13): the `!docs`/`!adrs` importers landed (`agents/plan/build/IMPORT-DOCS-ADRS.md`); never emitted, must not be reused |
| `OXZ-5004` | — | (tombstone) | retired at M5 (D9): `workspace extends` landed with file and JSON bases; never emitted, must not be reused |
| `OXZ-5005` | Error | import arrives in Phase 4 | `plantuml`/`mermaid`/`kroki` render branches and `theme <file>` content load parsed, importer deferred (D11, retired by P4); raster `image <file>` stays OXZ-5002 (D10) |
| `OXZ-5006` | Error | `cannot read/write …` | CLI I/O and input policy (M6): unreadable/unwritable file, stdio error, non-UTF-8 input (V-8: UTF-8 only), undetectable workspace format; exit code 3 |
| `OXZ-5007` | Error | export format not implemented | `export --format` parsed a reference-registry format whose exporter has not landed: `websequencediagrams` (out of Phase 2 scope), `static`/`png`/`svg` (not oxidizr targets) (P2-D10) — the Phase 2 formats `plantuml*`/`c4plantuml*`/`mermaid` landed at M2–M4; truly-unknown format strings are NOT a diagnostic — the reference logs `Unknown export format: …` and exits 0, oxidizr notes likewise |
| `OXZ-5008` | Error | theme loading failed | an installed theme's `theme.json` is unreadable/unparsable, a fetched theme's content-type is neither JSON nor plain text, or the fetch failed — including no system `curl` to perform it with `--allow-net` (P2-D3); a theme URL without `--allow-net` is `OXZ-5001`, not this code |
| `OXZ-5009` | Error | export aborted | the reference ABORTS the whole export where its JVM throws — the shared diagram engine's cast/NPE paths (`EXPORT-PLANTUML.md` §14): a view-kind cast mismatch, a dangling id dereferenced mid-write, a foreign animation frame; oxidizr reports the abort as this diagnostic instead of panicking (M2) |
| `OXZ-5010` | Error | `generate` failed | `generate system-landscape` (§P4-M3 O-4, P4-D9): an `--include`/`--exclude` pattern uses a construct outside the std-only full-match filter subset (`{m,n}` quantifiers, lookaround, `\p{…}`, named groups, inline flags — see `oxidizr_api::generate::pathfilter`), or the landscape cannot be derived (no workspace matched); never a silent widening or narrowing of the user's pattern |
