//! `oxidizr-dsl` — the DSL frontend (P1-D3): four hand-rolled stages
//! (`preprocess` → `lexer` → CST `parser` → `builder`) plus the trivia-preserving
//! formatter `fmt` (P1-D7) and the include/extends loader.
//!
//! Errors are values (`thiserror`) carrying spans and multi-error collectors;
//! "did you mean" suggestions come from the scope state machine's permitted-token
//! sets. No parser combinators — line-oriented tokenizer + recursive descent, zero
//! copies over the source.
// P1-M0 skeleton: manifest declares the agreed P1-D9 set before the code that uses
// it (parser/builder M4). `oxidizr-ast` is in use since M1 (CST); remove the rest of
// this allow as each milestone lands its code.
#![allow(unused_crate_dependencies)]

pub mod compat;
pub mod cst;
pub mod lexer;
pub mod preprocess;
