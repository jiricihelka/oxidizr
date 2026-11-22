# oxidizr-lsp

The language server for the Structurizr DSL: diagnostics, completion,
hover, and formatting over LSP/stdio.

A headless lib core (`server`: document store + message state machine,
pure data in / data out) plus a thin stdio binary (`main.rs` is the only
place that touches stdin/stdout). Editor wiring: `docs/lsp-editors.md`.

Consumes `oxidizr-dsl` + `oxidizr-diag` directly (fast re-parse per edit);
config-driven opens go through `oxidizr-api`.
