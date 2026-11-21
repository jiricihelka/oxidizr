//! `oxidizr-lsp` — the language server for the Structurizr DSL
//! (Phase 4, §P4-LSP, F5/P4-D5).
//!
//! The shape mirrors the viewer's shipped ritual (P3-D1/R-2): a
//! **headless lib core** ([`server`]: document store + message state
//! machine, pure data in / pure data out, Category A testable) plus a
//! **thin stdio binary** (`main.rs`, the ONLY place that touches
//! stdin/stdout). The wire is LSP over stdio via `lsp-server`
//! (sync, crossbeam, no tokio — the P4-D5 lean-deps decision).
//!
//! Sync model: **full-document sync** (P4-D5) — `didOpen`/`didChange`
//! replace the whole buffer; DSL files are small.
//!
//! Scope per item (§P4-LSP): L0 = this skeleton (handshake, sync,
//! transcript harness) → L1 diagnostics → L2 completion → L3 hover →
//! L4 formatting → L5 config + close.

pub mod completion;
pub mod diagnostics;
pub mod hover;
pub mod server;
