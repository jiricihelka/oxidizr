//! `oxidizr-dsl` — the DSL frontend: four hand-rolled stages
//! (`preprocess` → `lexer` → CST `parser` → `builder`) plus the trivia-preserving
//! formatter `fmt` and the include/extends loader.
//!
//! Errors are values (`thiserror`) carrying spans and multi-error collectors;
//! "did you mean" suggestions come from the scope state machine's permitted-token
//! sets. No parser combinators — line-oriented tokenizer + recursive descent, zero
//! copies over the source.
// The manifest declares the full dependency set ahead of the code that uses it;
// remove the rest of this allow as each dependency gets used.
#![allow(unused_crate_dependencies)]

pub mod compat;
pub mod cst;
pub mod engine;
pub mod fmt;
pub mod lexer;
pub mod preprocess;
pub mod query;
