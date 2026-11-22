# oxidizr-diag

Shared diagnostic plumbing: `Span` (file + byte range + line/col),
`Severity` (Error/Warning/Info), the `OXZ-nnnn` code newtype and the
machine-readable issue shape that reaches the CLI renderer and the LSP.

Code ranges: 1xxx lexer · 2xxx parser · 3xxx model & validation ·
4xxx JSON · 5xxx I/O & CLI policy · 9xxx internal. Every code is
registered in `codes::ALL` and documented in `docs/diagnostics.md`.

Consumed by every crate that reports problems.
