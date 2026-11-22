# oxidizr-dsl

The DSL frontend: four hand-rolled stages (`preprocess` → `lexer` → CST
`parser` → `builder`), the trivia-preserving formatter `fmt`, and the
include/extends loader.

No parser combinators — a line-oriented tokenizer plus recursive descent,
zero copies over the source. Errors are values carrying spans; "did you
mean" suggestions come from the parser's permitted-token sets. The
`query` module exposes the DSL item-expression grammar that powers
`oxidizr query`.

Consumed by `oxidizr-api`, `oxidizr-lsp` (formatter/diagnostics), and the
e2e suite.
