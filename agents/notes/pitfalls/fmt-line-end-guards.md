# fmt line-end guards (P1-D7)

Confirmed against the reference `StructurizrDslParser.preProcessLines` @ `ad34036`.

- The line contract is `dsl.split("\\r?\\n")` (the `parse(String)` route) —
  `parse(File)`'s `readAllLines` reader is NOT the contract; a CR before the
  emitted LF is a delimiter, so a content-final `\r` (only reachable via quoted
  values at EOF) must be DOUBLED (`X\r\r\n` re-splits to `X\r`).
- `endsWith("\\")` / `endsWith("\"\"\"")` run on the raw line behind a
  `!COMMENT_PATTERN` guard. A rendered body ending in `\` would silently join;
  ending in `"""` would open a text block. `"""`-end → one protective space;
  `\`-end → UNREPRESENTABLE (any emitted line ending re-joins; the space that
  hid it in the source cannot be re-created inside the value) → print the
  logical line's original bytes verbatim.
- Trailing whitespace is therefore NOT always droppable; trailing spaces that
  hid a `\`/`"""` end are load-bearing.
- Only the JOINER line of a `\`-chain is `stripLeading`ed — middle
  continuation lines keep their indent INSIDE the joined value, so the printer
  must test segment membership (a whitespace leaf inside a segment is a real
  gap; between-segment after a terminator it is stripped joiner indent).
- `lex_comment` regression: a `#` line joined onto an all-whitespace `\\`
  segment puts the opener in a LATER segment (fixed in lexer.rs, M6-b proptest;
  regression tests in lexer + fmt).
- Block-comment opener is re-printed through the code-line renderer (token
  identity keeps the trim-based region machine silent); region interior bytes
  are verbatim.
- `structurizr.dsl` (base64 source) and `structurizr.dsl.identifier` UUIDs make
  parse↔fmt(x) JSON comparisons non-deterministic — drop the property line,
  redact UUID shapes (see tests/m6_fmt.rs).
