# Style colors normalize on the DSL path too (T4 oracle verdict)

Why: the T4 invariant net caught a `dsl2json`→`json2json` byte instability
(uppercase hex survived DSL parse but lowercased on JSON re-read). The M4
reading "plain strings (no validation, Java)" was WRONG — the DSL importer
calls the same `Color` setter validation JSON hydration does.

Oracle verdicts (structurizr 2026.10.26, `$OXIDZR_STRUCTURIZR_BIN export
--format json`, probes kept at `/tmp/nix-shell.uni100/t4/oracle/`):

- `background #23A2D9` → exports `#23a2d9` (hex LOWERCASED on the DSL path).
- `background DarkSlateGray` → `#2f4f4f` (HTML name mapped to hex, 148-entry
  table, case-insensitive).
- `background bogusColor` → `StructurizrDslParserException: bogusColor is not
  a valid hex colour code or HTML colour name at line N of FILE: background
  bogusColor` — a STATEMENT error (our class: OXZ-2001). The DSL rendering
  drops the trailing period the JSON-side message keeps.

Fix shape: the port (`colors.rs` — table + `normalize`) moved
`oxidizr-json` → `oxidizr-core` (it is `com.structurizr.view.Color` domain
logic); `oxidizr-dsl/engine/stmt_style.rs` `style_color()` calls it at the
four color-set sites (element background/colour/stroke, relationship colour).
Rejections re-anchor: `white` → `#ffffff`, `green` → `#008000`.

Related oracle verdict from the same run: `url` inside `group` IS rejected by
the reference too (`Unexpected tokens …` at the `url` line) — the vendored
`testdata/dsl/group-url.dsl` is a NEGATIVE fixture; our engine matches byte
for byte in the message. Not a compat gap, no DEVIATIONS entry.

Testing: `OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e --test full`
(json-round-trip byte-stability is what surfaced it); `cargo test -p
oxidizr-dsl -p oxidizr-json` pin both paths.
