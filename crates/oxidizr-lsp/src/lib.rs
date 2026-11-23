//! `oxidizr-lsp` — the language server for the Structurizr DSL.
//!
//! A **headless lib core** ([`server`]: document store + message state
//! machine, pure data in / pure data out, testable with no client) plus
//! a **thin stdio binary** (`main.rs`, the ONLY place that touches
//! stdin/stdout). The wire is LSP over stdio via `lsp-server` — sync,
//! crossbeam, no tokio.
//!
//! Sync model: **full-document sync** — `didOpen`/`didChange` replace
//! the whole buffer; DSL files are small.
//!
//! What it answers: the `initialize`/`shutdown`/`exit` handshake,
//! diagnostics on open/change, completion, hover, the two formatting
//! requests, and `initializationOptions` ([`config`]).

pub mod completion;
pub mod config;
pub mod diagnostics;
pub mod formatting;
pub mod hover;
pub mod server;
