//! `oxidizr-ast` — the DSL concrete syntax tree: kind-tagged nodes with children,
//! trivia (comments/whitespace preserved for `fmt`) and spans. The losslessness
//! invariant (`Cst::text()` reproduces the source byte-for-byte, P1-D3) is defined
//! here and exercised by `oxidizr-dsl`.
//!
//! LSP (P4) consumes this crate + `oxidizr-diag` without pulling in the parser.
