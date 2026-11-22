# oxidizr-ast

The DSL concrete syntax tree: kind-tagged nodes with children, trivia
(comments and whitespace, preserved for the formatter) and byte spans.

The losslessness invariant is defined here: leaf text reconstructs the
original source byte-for-byte.

Key types: `Node`, `Cst`, `Span`-bearing leaves.

Consumed by `oxidizr-dsl` (parser/formatter) and `oxidizr-lsp` (which uses
this crate without pulling in the parser).
