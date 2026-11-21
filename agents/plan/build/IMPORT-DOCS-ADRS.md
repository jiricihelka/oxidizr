# IMPORT-DOCS-ADRS — the `!docs`/`!adrs` importer port (P4 §P4-M2 N-6)

P4-V7 and P4-V8 CLOSED 2026-11-21: every fact below was read from the pinned
monorepo `ad34036` (app 2026.10.26 / lib 6.2.4) — `structurizr-import`
`documentation/*`, `structurizr-dsl` `DocsParser`/`DecisionsParser`/
`DocumentationDslContext`/`DecisionsDslContext`/`StructurizrDslParser`/
`DslContext.loadClass`, `structurizr-core` `documentation/*` + `ImageUtils`,
`structurizr-client` `AbstractJsonWriter`. Crate split: the importers live in
`oxidizr-import` (`docs.rs`, `decisions.rs`, `regexlite.rs`); `oxidizr-dsl`
depends on `oxidizr-import` (acyclic — import deps = core + base64 + thiserror)
and runs the import at the context `end()` exactly where the OXZ-5003 stub sat.

## V8 CLOSED — exclude matching is FULL-MATCH REGEX, not glob

`DefaultDocumentationImporter.excluded` / `AbstractDecisionImporter.excluded`
(both copies): `file.getName().equals(regex) || file.getName().matches(regex)`
— `String.matches` = Java regex FULL MATCH. `globset` therefore does NOT
return; `oxidizr-import::regexlite` is a std-only backtracking matcher over
the Java-subset the corpus + reference internals need: literals, `.` (NOT
newline), escapes (`\.` `\d` `\w` `\s` `\\` …), classes `[abc]`/`[^abc]` with
ranges, groups `(...)` with capture (groups 1/2 feed `findImages` + the ADR
LINK regexes), alternation, greedy + lazy `* + ? {n} {n,m} {n,}`. API:
`Regex::new(&str) -> Result`, `is_match_full(s)` (`matches`), `find_iter(s)`
yielding group captures (leftmost-first, then per position). Reference
patterns ported through the SAME engine (byte-parity by construction):
`findImages` Markdown `!\[.*]\((.*)\)` / AsciiDoc `image:{1,2}(.*)\[.*]`,
AdrTools `(.*) \[.*]\((.*)\)`, Madr `\[.*]\((.*)\)`, Log4brains
`- Status: (.*) \[.*]\((.*)\)` and `- (.*) \[.*]\((.*)\)`, Madr filename
filter `\d{4}-.+?.md`, Log4brains `\d{4}\d{2}\d{2}-.+?.md`.
Lazy-as-greedy is acceptable ONLY where no trailing context pins the match
(all patterns above end greedy-anchored); the engine implements real lazy
backtracking anyway. The Madr front-matter regex `(?m)^---\n(^.*\n)*?---\n`
is NOT engine-fed — `decisions.rs` strips the block procedurally (scan for
line-start `---\n`, consume to the next line-start `---\n`), documented so.

## V7 CLOSED — decision parsing per tool (pinned behavior)

Common (`AbstractDecisionImporter`): excludes regex as above; dates parsed
with `SimpleDateFormat` + `TimeZone.getDefault()` — THE REFERENCE IS
MACHINE-TZ DEPENDENT (this box = CET+0100). Port parses UTC (std-only, no tz
database) ⇒ DEVIATION D15; oracle legs run with `TZ=UTC`. Content read
UTF-8; `Decision.setContent` normalizes `\r\n|\r|\n`→`\n`
(`replaceAll("\\r\\n|\\r|\\n","\n")` — AdrTools/Madr/Log4brains additionally
`content.replace("\r","")` BEFORE their own parsing). `Documentation.addSection`
assigns `order = sections.size()+1` AT ADD TIME; `addDecision` validates
id/title/content/status non-null + id uniqueness ("A decision with an ID of
X already exists in this scope."), images plain append. JSON: date = epoch
SECONDS; `AbstractJsonWriter` pins its formatter to **UTC** — export-side
determinism confirmed. `Decision.Link` = `{id, description}`, self-links
dropped (`addLink` no-ops when ids equal). URL rewrite (all three importers,
post-import): every occurrence of a decision FILENAME inside every decision's
content is replaced by `#` + urlEncode(id) (`URLEncoder`, `+`→`%20`; ids are
digits ⇒ plain).

- **AdrTools** (`…documentation.AdrToolsDecisionImporter`, `!adrs` DEFAULT):
  dir-only; `listFiles(endsWith(".md"))` — NOT sorted (see determinism note);
  id = `Integer.parseInt(name[0..4])` (bad prefix ⇒ whole import aborts);
  title = first line, `substring(indexOf(".")+2)` (no dot ⇒ drops first
  char); date = first line starting `Date: ` parsed `yyyy-MM-dd` (LENIENT,
  machine-TZ ⇒ UTC port), absent ⇒ `new Date()` (NOW — non-reproducible;
  fixtures must carry `Date:` lines); status = first non-empty line after a
  `## Status` line, `split(" ")[0]` first word, `Superceded`→`Superseded`,
  absent ⇒ `proposed`; links = status-section lines until `## Context`
  matching `(.*) \[.*]\((.*)\)` (group1 = description, target must be a
  KNOWN FILENAME) — description carries the line prefix.
- **Madr** (`…MadrDecisionImporter`, `!adrs madr`): dir-only; filename filter
  full-match `\d{4}-.+?.md`; SORTED by name; id = `parseInt(name[0..4])`;
  front matter (`---` block) STRIPPED for content/title; title = first
  `# `-prefixed line of stripped content (default `"Title"`); date = file
  mtime, overridden by front-matter `date: yyyy-MM-dd`; status =
  front-matter `status: ` rest-of-line, default `accepted`; links = standard
  Markdown links over the FULL (stripped) content, description fixed
  `"Links to"`; per-file failure aborts (`"Error importing decision from + …"`).
- **Log4brains** (`…Log4brainsDecisionImporter`, `!adrs log4brains`):
  dir-only; filter `\d{4}\d{2}\d{2}-.+?.md`; SORTED; ids SEQUENTIAL from 1
  (skips count excluded files — the counter still advances ONLY for imported
  ones — pinned: increment sits inside the import branch); title = FIRST
  LINE `substring(2)` (no `#` check); date = filename `yyyyMMdd`, overridden
  by first `- Date: ` line; status = first `- Status: ` rest-of-line
  (default `""`), leading `superseded` normalized to `superseded`; links =
  `- Status: (.*) \[…]\((.*)\)` group1/2 + `## Links` section lines matching
  `- (.*) \[…]\((.*)\)`.
- **Structurizr-native decisions**: NO such importer exists in the pinned
  monorepo (importer package inventory = Default{Documentation,Image},
  RecursiveDefaultDocumentation, Abstract/AdrTools/Madr/Log4brains,
  FormatFinder) — P4-D13's "4th format" is a non-goal.

## Importer behaviors (byte contract inputs)

- **`!docs` default = `DefaultDocumentationImporter`**: NON-RECURSIVE.
  `!docs`-default and both DSL forms share: path = `new File(dslFile.parent,
  token)` (ALWAYS joined — an absolute-looking token still joins, Java
  `File(parent,child)` semantics); `dslFile == null` ⇒ import silently
  SKIPPED (our `base_dir: None` — stdin/string parses). Path absent ⇒
  `"<abs> does not exist."` → DSL error `"Error importing documentation: …"`
  / `"Error importing decisions: …"` (unknown FQN ⇒ `"Error importing
  documentation: <fqn> was not found"` — loadClass runs BEFORE the exists
  check; the FQN is resolved against a plugin-jar URLClassLoader — JVM-only,
  our registry = the six built-in FQNs + the `adrtools`/`madr`/`log4brains`
  aliases, everything else "was not found"). Dir import: `listFiles()` sorted
  by path, skip directories, skip dot-prefixed names, per file: extension ∈
  {`.md .markdown .text`}→Markdown / {`.asciidoc .adoc .asc`}→AsciiDoc
  (FormatFinder, case-SENSITIVE, extension = from LAST dot) and not
  excluded ⇒ section {content UTF-8, filename}. Single file: imported
  REGARDLESS of extension (FormatFinder "just assume Markdown"). Post-import
  filename trim: `canonical.replace(base.canonical, "")` minus one leading
  separator (single-file form ⇒ filename `""`); recursive variant also maps
  `\`→`/`. Then, IF path was a directory: reluctant `DefaultImageImporter`
  runs on the SAME documentable+dir.
- **`RecursiveDefaultDocumentationImporter`** (FQN-only form): same,
  descending directories (dot-dirs not special-cased — Java checks
  `isDirectory` first, hidden dirs recurse).
- **Reluctant `DefaultImageImporter`**: images referenced by `findImages`
  across the documentable's sections THEN decisions (Markdown vs AsciiDoc
  pattern by content `format`, null format ⇒ AsciiDoc branch); dir walk
  RECURSIVE but NOT sorted (FS order — determinism note), subdirectory names
  join with `/` (`sub/dir/file.png`), `.png .jpg .jpeg .gif .svg` lowercased
  suffixes; content = base64 standard alphabet WITHOUT data-URI prefix (the
  `Image` bean stores `{name, content, type}`); SVG raw bytes = the ONLY
  reference-deterministic format — non-SVG re-encodes via ImageIO ⇒
  deviation D16 (raw bytes).
- DSL surface: `!docs <path> [fqn]` (default DefaultDocumentationImporter),
  `!adrs`/`!decisions <path> [type|fqn]` (default `adrtools`; aliases
  case-SENSITIVE `getOrDefault`), `exclude <filename|regex>…` accumulates on
  the context; block ends = import time; non-portable frames (already wired).

## Determinism notes (semantic bar, F12-adjacent)

- AdrTools `listFiles` is UNSORTED (FS order) + rewrite map is a HashMap —
  the reference itself is non-deterministic there; the port sorts by name
  (matches Madr/Log4brains explicit sorting). Byte parity vs oracle import
  output holds only when the FS order agrees; our goldens are OUR blessed
  output, the read gate (reference re-export of our JSON) is the byte gate.
- AdrTools files without a `Date:` line take `new Date()` (NOW): fixtures
  always carry dates; unit tests assert around it explicitly.
- Madr files without front-matter `date:` take the file mtime: same rule —
  fixtures carry `date:`; the vendored corpus is exercised by unit tests
  that skip those specific date fields, blessed goldens run on fixtures we
  author.

## Corpus + oracle protocol

- Vendored corpora (already present, byte-exact): `testdata/dsl/decisions/
  {adrtools,madr,log4brains}/` + `testdata/dsl/docs/` (reference test
  resources). Deterministic SUCCESS fixtures are authored under
  `testdata/dsl/` for the e2e corpus + CLI goldens; the two deferred
  fixtures (`docs/workspace.dsl`, `decisions/workspace.dsl`) KEEP their
  unknown-FQN arms — they remain invalid-corpus fixtures whose registered
  first-error changes from the OXZ-5003 text to the reference-shaped "was
  not found" statement error.
- Oracle read gate: `oxidizr dsl2json X.dsl -o X.json` then
  `TZ=UTC $OXIDZR_STRUCTURIZR_BIN export -w X.json --format json` byte-compares.
  Reference IMPORT capture (`export -w X.dsl` parses the DSL directly) runs
  with `TZ=UTC` for decision-date fixtures (D15).
- OXZ-5003 retires as a tombstone exactly like 5004: constant + catalog rows
  (`docs/diagnostics.md`, coverage REGISTRY) removed; `KNOWN_ERRORS` entries
  in `m4_fixtures.rs` and the deferred-fixture list in `testdata/README.md`
  retired/updated with N-6.
